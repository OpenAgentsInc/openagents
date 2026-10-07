use crate::{
    Algorithm, Attribution, Cancellation, Completeness, Config, Coverage, DISCOVERY_PATH,
    Discovery, Error, HOUSE_PATH, HouseIdentity, Influence, Observation, Operation, RANK_PATH,
    SEARCH_PATH, Subject, bounded_json, digest, pubkey,
    transport::{Gate, Transport},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Semaphore, watch};
use url::Url;

#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    config: Config,
    config_digest: String,
    transport: Transport,
    gate: watch::Sender<Gate>,
    slots: Semaphore,
}

impl Client {
    /// Create a disabled client. Construction and enablement perform no reads.
    pub fn new(config: Config) -> Result<Self, Error> {
        Self::build(config, false)
    }

    fn build(mut config: Config, fixture_http: bool) -> Result<Self, Error> {
        config.limits.validate()?;
        if config.limits.search_results > config.limits.rank_subjects {
            return Err(Error::InvalidConfiguration {
                field: "search_results".into(),
            });
        }
        let origin = Url::parse(&config.origin).map_err(|_| Error::InvalidConfiguration {
            field: "origin".into(),
        })?;
        if (origin.scheme() != "https"
            && !(fixture_http
                && origin.scheme() == "http"
                && origin.host_str() == Some("127.0.0.1")))
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.path() != "/"
        {
            return Err(Error::InvalidConfiguration {
                field: "origin".into(),
            });
        }
        config.origin = origin.origin().ascii_serialization();
        let config_digest = digest(&bounded_json(&config, 4096)?);
        let gate = watch::channel(Gate {
            enabled: false,
            epoch: 0,
        })
        .0;
        let transport = Transport::new(
            origin,
            gate.clone(),
            config.limits.response_bytes,
            config.limits.cache_ttl_seconds,
        )?;
        let slots = Semaphore::new(config.limits.concurrent_operations);
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                config_digest,
                transport,
                gate,
                slots,
            }),
        })
    }

    #[cfg(test)]
    pub(crate) fn fixture(config: Config) -> Result<Self, Error> {
        Self::build(config, true)
    }

    pub fn configuration(&self) -> &Config {
        &self.inner.config
    }
    pub fn is_enabled(&self) -> bool {
        self.inner.gate.borrow().enabled
    }

    /// Disable invalidates in-flight operations, including disable/enable races.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.gate.send_modify(|gate| {
            if gate.enabled != enabled {
                gate.enabled = enabled;
                gate.epoch = gate.epoch.wrapping_add(1);
            }
        });
    }

    pub async fn discover(&self, cancellation: &Cancellation) -> Result<Discovery, Error> {
        self.run(cancellation, |epoch| self.discovery(epoch, cancellation))
            .await
    }

    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        cancellation: &Cancellation,
    ) -> Result<Observation, Error> {
        let bounds = &self.inner.config.limits;
        if query.trim().is_empty()
            || query.chars().count() > bounds.query_characters
            || query.len() > bounds.query_bytes
        {
            return Err(Error::InvalidInput {
                field: "query".into(),
            });
        }
        if limit == 0 || limit > bounds.search_results {
            return Err(Error::InvalidInput {
                field: "limit".into(),
            });
        }
        let body = bounded_json(
            &serde_json::json!({ "query": query, "algorithm": "relevance", "limit": limit }),
            8192,
        )?;
        self.run(cancellation, |epoch| async move {
            let discovery = self.discovery(epoch, cancellation).await?;
            if !discovery.search_supported {
                return Err(Error::UnsupportedOperation {
                    endpoint: SEARCH_PATH.into(),
                });
            }
            let response = self
                .inner
                .transport
                .request(
                    SEARCH_PATH,
                    Some(body.clone()),
                    Some(Algorithm::Relevance),
                    epoch,
                    cancellation,
                )
                .await?;
            if let Some(error) = response.error {
                return Err(error);
            }
            let rows = rows(&response.bytes, limit, None)?;
            let mut responses = discovery.responses.clone();
            responses.push(response.evidence);
            let mut subjects: Vec<_> = rows
                .iter()
                .map(|row| subject(&row.pubkey, Some(row.rank), None))
                .collect();
            let mut enrichment_error = None;
            if !subjects.is_empty() {
                let keys: Vec<_> = subjects
                    .iter()
                    .map(|subject| subject.pubkey.clone())
                    .collect();
                let enrichment = if discovery.rank_supported {
                    self.rank_read(&keys, epoch, cancellation).await
                } else {
                    Err((
                        Error::UnsupportedOperation {
                            endpoint: RANK_PATH.into(),
                        },
                        None,
                    ))
                };
                match enrichment {
                    Ok((ranks, evidence)) => {
                        let ranks: HashMap<_, _> = ranks
                            .into_iter()
                            .map(|row| (row.pubkey, influence(row.rank)))
                            .collect();
                        for subject in &mut subjects {
                            subject.influence = ranks.get(&subject.pubkey).cloned();
                        }
                        responses.push(evidence);
                    }
                    Err((error @ (Error::Disabled | Error::Cancelled | Error::Timeout), _)) => {
                        return Err(error);
                    }
                    Err((error, evidence)) => {
                        enrichment_error = Some(error);
                        if let Some(evidence) = evidence {
                            responses.push(evidence);
                        }
                    }
                }
            }
            self.observation(
                Operation::SearchPeople,
                &body,
                discovery.house,
                subjects,
                responses,
                enrichment_error,
            )
        })
        .await
    }

    /// Rank canonical public keys. Display-name resolution is a separate task.
    pub async fn rank(
        &self,
        keys: &[String],
        cancellation: &Cancellation,
    ) -> Result<Observation, Error> {
        let keys = self.keys(keys)?;
        let body = rank_body(&keys)?;
        self.run(cancellation, |epoch| async move {
            let discovery = self.discovery(epoch, cancellation).await?;
            if !discovery.rank_supported {
                return Err(Error::UnsupportedOperation {
                    endpoint: RANK_PATH.into(),
                });
            }
            let (rows, evidence) = self
                .rank_read(&keys, epoch, cancellation)
                .await
                .map_err(|(error, _)| error)?;
            let found: HashMap<_, _> = rows
                .into_iter()
                .map(|row| (row.pubkey, influence(row.rank)))
                .collect();
            let subjects = keys
                .iter()
                .map(|key| subject(key, None, found.get(key).cloned()))
                .collect();
            let mut responses = discovery.responses;
            responses.push(evidence);
            self.observation(
                Operation::Rank,
                &body,
                discovery.house,
                subjects,
                responses,
                None,
            )
        })
        .await
    }

    fn keys(&self, keys: &[String]) -> Result<Vec<String>, Error> {
        if keys.is_empty() || keys.len() > self.inner.config.limits.rank_subjects {
            return Err(Error::InvalidInput {
                field: "pubkeys".into(),
            });
        }
        let mut found = HashSet::new();
        keys.iter()
            .map(|key| {
                let key = pubkey(key).ok_or_else(|| Error::InvalidInput {
                    field: "pubkeys".into(),
                })?;
                if !found.insert(key.clone()) {
                    return Err(Error::InvalidInput {
                        field: "duplicate_pubkey".into(),
                    });
                }
                Ok(key)
            })
            .collect()
    }

    async fn run<T, F, Fut>(&self, cancellation: &Cancellation, operation: F) -> Result<T, Error>
    where
        F: FnOnce(u64) -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        let snapshot = *self.inner.gate.borrow();
        if !snapshot.enabled {
            return Err(Error::Disabled);
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut gate = self.inner.gate.subscribe();
        let work = async {
            let _slot = self
                .inner
                .slots
                .acquire()
                .await
                .map_err(|_| Error::Disabled)?;
            operation(snapshot.epoch).await
        };
        tokio::select! {
            biased;
            _ = gate.wait_for(|gate| !gate.enabled || gate.epoch != snapshot.epoch) => Err(Error::Disabled),
            _ = cancellation.cancelled() => Err(Error::Cancelled),
            result = tokio::time::timeout(Duration::from_millis(self.inner.config.limits.deadline_ms), work) =>
                result.unwrap_or(Err(Error::Timeout)),
        }
    }

    async fn discovery(&self, epoch: u64, cancellation: &Cancellation) -> Result<Discovery, Error> {
        let response = self
            .inner
            .transport
            .request(DISCOVERY_PATH, None, None, epoch, cancellation)
            .await
            .map_err(|error| discovery_error("capabilities", error))?;
        if let Some(error) = response.error {
            return Err(discovery_error("capabilities", error));
        }
        let capabilities: HashMap<String, Vec<Advertised>> =
            serde_json::from_slice(&response.bytes).map_err(|_| {
                discovery_error(
                    "capabilities",
                    Error::InvalidResponse {
                        field: "capabilities".into(),
                    },
                )
            })?;
        let supports = |path: &str, algorithm: Algorithm| {
            capabilities.get(path).is_some_and(|algorithms| {
                algorithms
                    .iter()
                    .any(|advertised| advertised.id == algorithm.wire() && !advertised.pov)
            })
        };
        let search_supported = supports(SEARCH_PATH, Algorithm::Relevance);
        let rank_supported = supports(RANK_PATH, Algorithm::Graperank);
        let house = self
            .inner
            .transport
            .request(HOUSE_PATH, None, None, epoch, cancellation)
            .await
            .map_err(|error| discovery_error("house_identity", error))?;
        if let Some(error) = house.error {
            return Err(discovery_error("house_identity", error));
        }
        #[derive(Deserialize)]
        struct Names {
            names: HashMap<String, String>,
        }
        let names: Names = serde_json::from_slice(&house.bytes).map_err(|_| {
            discovery_error(
                "house_identity",
                Error::InvalidResponse {
                    field: "house_identity".into(),
                },
            )
        })?;
        let key = names
            .names
            .get("_")
            .and_then(|key| pubkey(key))
            .ok_or_else(|| {
                discovery_error(
                    "house_identity",
                    Error::InvalidResponse {
                        field: "house_identity".into(),
                    },
                )
            })?;
        let house_identity = HouseIdentity {
            pubkey: key,
            origin: self.inner.config.origin.clone(),
            discovered_at_ms: house.evidence.fetched_at_ms,
            attribution: Attribution::SeparateHttpsObservation,
        };
        let expires_at_ms = response
            .evidence
            .expires_at_ms
            .min(house.evidence.expires_at_ms);
        let discovery = Discovery {
            house: house_identity,
            search_supported,
            rank_supported,
            responses: vec![response.evidence, house.evidence],
            expires_at_ms,
        };
        bounded_json(&discovery, self.inner.config.limits.normalized_bytes)?;
        Ok(discovery)
    }

    async fn rank_read(
        &self,
        keys: &[String],
        epoch: u64,
        cancellation: &Cancellation,
    ) -> Result<(Vec<Row>, crate::ResponseEvidence), (Error, Option<crate::ResponseEvidence>)> {
        let body = rank_body(keys).map_err(|error| (error, None))?;
        let response = self
            .inner
            .transport
            .request(
                RANK_PATH,
                Some(body),
                Some(Algorithm::Graperank),
                epoch,
                cancellation,
            )
            .await
            .map_err(|error| (error, None))?;
        if let Some(error) = response.error {
            return Err((error, Some(response.evidence)));
        }
        let rows = rows(&response.bytes, keys.len(), Some(keys))
            .map_err(|error| (error, Some(response.evidence.clone())))?;
        Ok((rows, response.evidence))
    }

    fn observation(
        &self,
        operation: Operation,
        input: &[u8],
        house: HouseIdentity,
        subjects: Vec<Subject>,
        responses: Vec<crate::ResponseEvidence>,
        enrichment_error: Option<Error>,
    ) -> Result<Observation, Error> {
        let completeness = if enrichment_error.is_some()
            || subjects.iter().any(|subject| subject.influence.is_none())
        {
            Completeness::Partial
        } else {
            Completeness::Bounded
        };
        let mut expires_at_ms = responses
            .iter()
            .map(|response| response.expires_at_ms)
            .min()
            .unwrap_or(0);
        // A failed enrichment has no TTL. Do not cache its partial observation.
        if enrichment_error.is_some() {
            expires_at_ms = expires_at_ms.min(crate::now_ms());
        }
        let observation = Observation {
            operation,
            configuration: self.inner.config.clone(),
            configuration_digest: self.inner.config_digest.clone(),
            input_digest: digest(input),
            house,
            subjects,
            responses,
            enrichment_error,
            completeness,
            expires_at_ms,
        };
        bounded_json(&observation, self.inner.config.limits.normalized_bytes)?;
        Ok(observation)
    }
}

#[derive(Deserialize)]
struct Advertised {
    id: String,
    #[serde(default)]
    pov: bool,
}

#[derive(Deserialize)]
struct Row {
    pubkey: String,
    rank: f64,
}

#[derive(Deserialize)]
struct Data {
    results: Vec<Row>,
    ttl: Option<u64>,
}

fn rows(bytes: &[u8], maximum: usize, requested: Option<&[String]>) -> Result<Vec<Row>, Error> {
    let mut data: Data = serde_json::from_slice(bytes).map_err(|_| Error::InvalidResponse {
        field: "results_or_ttl".into(),
    })?;
    // Deserializing the TTL also rejects negative, fractional, or string TTLs.
    let _ttl = data.ttl;
    if data.results.len() > maximum {
        return Err(Error::InvalidResponse {
            field: "result_count".into(),
        });
    }
    let mut found = HashSet::new();
    for row in &mut data.results {
        row.pubkey = pubkey(&row.pubkey).ok_or_else(|| Error::InvalidResponse {
            field: "pubkey".into(),
        })?;
        if !row.rank.is_finite() {
            return Err(Error::InvalidResponse {
                field: "rank".into(),
            });
        }
        if !found.insert(row.pubkey.clone()) {
            return Err(Error::InvalidResponse {
                field: "duplicate_pubkey".into(),
            });
        }
        if requested.is_some_and(|keys| !keys.contains(&row.pubkey)) {
            return Err(Error::InvalidResponse {
                field: "unrequested_pubkey".into(),
            });
        }
    }
    Ok(data.results)
}

fn rank_body(keys: &[String]) -> Result<Vec<u8>, Error> {
    bounded_json(
        &serde_json::json!({ "pubkeys": keys, "algorithm": "graperank" }),
        8192,
    )
}

fn influence(value: f64) -> Influence {
    Influence {
        value,
        coverage: if value == 0.0 {
            Coverage::Unknown
        } else {
            Coverage::Reported
        },
    }
}

fn subject(key: &str, relevance: Option<f64>, influence: Option<Influence>) -> Subject {
    Subject {
        pubkey: key.into(),
        profile_url: format!("https://njump.me/{key}"),
        relevance,
        influence,
    }
}

fn discovery_error(component: &str, error: Error) -> Error {
    match error {
        Error::Disabled | Error::Cancelled | Error::Timeout => error,
        cause => Error::DiscoveryUnavailable {
            component: component.into(),
            cause: Box::new(cause),
        },
    }
}
