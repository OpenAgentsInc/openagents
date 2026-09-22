//! Revised NIP-CAP v1 definitions, preferences, and discovery tags.
//!
//! A definition describes an interface. It does not grant use of a binding,
//! and a support value of `enforced` is a claim until an enforcement plan
//! names the host mechanism or the trusted executor contract that covers it.
//! Omitted bounds are `unknown`.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::contracts::{
    ArtifactRef, BoundAssignment, ContractError, Effects, RefusalCode, check_enforcement,
    parse_artifact, parse_effects, parse_strict,
};
use crate::domain::{Event, Tag};

/// Addressable capability discovery.
pub const DISCOVERY_KIND: u16 = 30_180;
/// Addressable operator preference.
pub const PREFERENCE_KIND: u16 = 30_181;
/// Public definition marker.
pub const CAP_MARKER: &str = "oa:cap:v1";
/// Public preference marker.
pub const PUBLIC_POLICY_MARKER: &str = "oa:cap-policy:public:v1";
/// Private preference marker. The `d` tag is a random mailbox, not a machine name.
pub const PRIVATE_POLICY_MARKER: &str = "oa:cap-policy:private:v1";

const PUBLISHER_HEX: usize = 64;

/// Which implementation shape a definition describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// A stable host operation.
    Native,
    /// An executable adapter.
    Executor,
    /// A guest plugin.
    Plugin,
    /// A remote or local operation adapter.
    Adapter,
    /// A remote inference or decision service.
    Service,
}

impl Profile {
    /// The `oa:profile:` tag value.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Native => "oa:profile:native",
            Self::Executor => "oa:profile:executor",
            Self::Plugin => "oa:profile:plugin",
            Self::Adapter => "oa:profile:adapter",
            Self::Service => "oa:profile:service",
        }
    }

    fn parse(value: &str) -> Result<Self, ContractError> {
        Ok(match value {
            "native" => Self::Native,
            "executor" => Self::Executor,
            "plugin" => Self::Plugin,
            "adapter" => Self::Adapter,
            "service" => Self::Service,
            _ => {
                return Err(ContractError::new(
                    RefusalCode::UnsupportedFeature,
                    "profile",
                ));
            }
        })
    }
}

/// What a definition says about one common bound. Absence is [`Support::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// The binding claims a mechanism.
    Enforced,
    /// The binding says it will not hold the bound.
    NotEnforced,
    /// The definition does not say.
    Unknown,
}

/// A portable capability definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// Qualified component id.
    pub id: String,
    /// Package component slug.
    pub component: String,
    /// Implementation profile.
    pub profile: Profile,
    /// Inert selector text.
    pub summary: String,
    /// Declared effects. Empty denies that class.
    pub effects: Effects,
    /// Stated common bounds. Missing names are unknown.
    pub bounds: BTreeMap<String, Support>,
    /// Cancellation mechanism name. Not a promise of remote stop.
    pub cancellation: String,
    /// Idempotency claim for this binding.
    pub idempotency: String,
    /// Transport named by the binding contract.
    pub transport: String,
    /// Isolation modes the contract accepts.
    pub isolation: Vec<String>,
    /// Minimum ceilings. Each must be covered before admission.
    pub minimum: BTreeMap<String, u64>,
}

/// An operator preference. It breaks ties. It does not grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preference {
    /// Ordered qualified ids.
    pub prefer: Vec<String>,
    /// Qualified ids the operator denies.
    pub deny: Vec<String>,
    /// Host or trusted-contract assurance.
    pub assurance: String,
}

/// A host grant. A definition that carries one is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// Principal pubkey.
    pub principal: String,
    /// Qualified binding id.
    pub binding: String,
    /// Logical scopes.
    pub scope: Vec<String>,
}

/// Parse a capability definition. `binding` is not a field of this object.
///
/// # Errors
///
/// Returns a typed refusal for an unknown field, profile, or uncovered minimum.
pub fn parse_definition(value: &Value) -> Result<Definition, ContractError> {
    let object = as_map(value, "definition")?;
    reject(
        object,
        &[
            "v",
            "requires",
            "id",
            "profile",
            "summary",
            "input",
            "output",
            "effects",
            "minimum",
            "support",
            "binding_contract",
            "meta",
        ],
        "definition",
    )?;
    version(object, "definition")?;
    let id = qualified(require(object, "id", "definition")?)?;
    let component = id.split('/').nth(1).unwrap_or("").to_string();
    let profile = Profile::parse(text(require(object, "profile", "definition")?, "profile")?)?;
    let summary = text(require(object, "summary", "definition")?, "summary")?.to_string();
    if summary.is_empty() {
        return Err(malformed("summary"));
    }
    let _input = parse_schema(require(object, "input", "definition")?)?;
    let _output = parse_schema(require(object, "output", "definition")?)?;
    let effects = parse_effects(require(object, "effects", "definition")?)?;
    let minimum = minimum(require(object, "minimum", "definition")?)?;
    let support = as_map(require(object, "support", "definition")?, "support")?;
    reject(
        support,
        &["bounds", "cancellation", "idempotency", "evidence"],
        "support",
    )?;
    let bounds = support_bounds(require(support, "bounds", "support")?)?;
    let cancellation = enum_text(
        require(support, "cancellation", "support")?,
        &[
            "before_dispatch",
            "cooperative",
            "host_terminated",
            "unsupported",
        ],
        "cancellation",
    )?;
    let idempotency = enum_text(
        require(support, "idempotency", "support")?,
        &["none", "request_attempt", "pure"],
        "idempotency",
    )?;
    let _evidence = string_list(require(support, "evidence", "support")?, "evidence")?;
    let contract = as_map(
        require(object, "binding_contract", "definition")?,
        "binding_contract",
    )?;
    let (transport, isolation) = binding_contract(profile, contract)?;
    let definition = Definition {
        id,
        component,
        profile,
        summary,
        effects,
        bounds,
        cancellation,
        idempotency,
        transport,
        isolation,
        minimum,
    };
    Ok(definition)
}

/// The support recorded for `bound`. A name the definition omits is unknown.
#[must_use]
pub fn bound_support(definition: &Definition, bound: &str) -> Support {
    definition
        .bounds
        .get(bound)
        .copied()
        .unwrap_or(Support::Unknown)
}

/// Refuse a minimum the plan does not cover.
///
/// `not_enforced` and `unknown` cannot satisfy themselves. Coverage is a
/// host mechanism or an explicitly trusted executor contract in `plan`.
///
/// # Errors
///
/// Returns [`RefusalCode::CannotEnforce`] when a minimum has no assignment.
pub fn cover(definition: &Definition, plan: &[BoundAssignment]) -> Result<(), ContractError> {
    for name in definition.minimum.keys() {
        let support = bound_support(definition, name);
        let assigned = plan.iter().any(|item| item.bound_name() == name);
        if !assigned || matches!(support, Support::NotEnforced | Support::Unknown) && !assigned {
            return Err(ContractError::new(
                RefusalCode::CannotEnforce,
                format!("minimum {name}"),
            ));
        }
        if matches!(support, Support::NotEnforced | Support::Unknown) {
            let host = plan.iter().any(|item| {
                item.bound_name() == name
                    && matches!(item.assurance, crate::contracts::Assurance::Host)
            });
            if !host {
                return Err(ContractError::new(
                    RefusalCode::CannotEnforce,
                    format!("minimum {name} needs a host mechanism"),
                ));
            }
        }
    }
    let required: Vec<crate::contracts::Bound> = plan.iter().map(|item| item.bound).collect();
    check_enforcement(&required, plan)?;
    Ok(())
}

/// `t` tags a public discovery event must carry. They agree with the definition.
///
/// A `service` definition gets its lane transports from
/// [`service_tags`]; this returns the marker and profile tags only.
#[must_use]
pub fn discovery_tags(definition: &Definition) -> Vec<Tag> {
    let mut tags = vec![
        Tag::new(vec!["t".into(), CAP_MARKER.into()]),
        Tag::new(vec!["t".into(), definition.profile.tag().into()]),
    ];
    if definition.profile != Profile::Service {
        tags.push(Tag::new(vec![
            "t".into(),
            format!("oa:transport:{}", definition.transport),
        ]));
    }
    tags
}

/// `t` tags a public service manifest carries: the marker, the `service`
/// profile, and one `oa:transport:` tag per distinct lane transport.
#[must_use]
pub fn service_tags(contract: &ServiceContract) -> Vec<Tag> {
    let mut transports: Vec<&str> = contract
        .lanes
        .iter()
        .map(|lane| lane.transport.as_str())
        .collect();
    transports.sort_unstable();
    transports.dedup();
    let mut tags = discovery_tags_for(Profile::Service);
    for transport in transports {
        tags.push(Tag::new(vec![
            "t".into(),
            format!("oa:transport:{transport}"),
        ]));
    }
    tags
}

fn discovery_tags_for(profile: Profile) -> Vec<Tag> {
    vec![
        Tag::new(vec!["t".into(), CAP_MARKER.into()]),
        Tag::new(vec!["t".into(), profile.tag().into()]),
    ]
}

/// Refuse discovery tags that duplicate or disagree with `definition`.
///
/// # Errors
///
/// Returns [`RefusalCode::IdentityMismatch`] when a tag names another profile
/// or transport.
pub fn check_discovery_tags(tags: &[Tag], definition: &Definition) -> Result<(), ContractError> {
    let mut marker = 0;
    let mut profile = 0;
    let mut transport = 0;
    let mut service_transports: Vec<&str> = Vec::new();
    for tag in tags {
        if tag.name() != Some("t") {
            continue;
        }
        let value = tag.value().unwrap_or("");
        if value == CAP_MARKER {
            marker += 1;
        } else if value == definition.profile.tag() {
            profile += 1;
        } else if value == format!("oa:transport:{}", definition.transport)
            && definition.profile != Profile::Service
        {
            transport += 1;
        } else if let Some(lane) = value.strip_prefix("oa:transport:") {
            if definition.profile != Profile::Service || service_transports.contains(&lane) {
                return Err(ContractError::new(
                    RefusalCode::IdentityMismatch,
                    "discovery tag",
                ));
            }
            service_transports.push(lane);
        } else if value.starts_with("oa:profile:") {
            return Err(ContractError::new(
                RefusalCode::IdentityMismatch,
                "discovery tag",
            ));
        }
    }
    if marker != 1 || profile != 1 {
        return Err(malformed("discovery tags"));
    }
    if definition.profile != Profile::Service && transport != 1 {
        return Err(malformed("discovery tags"));
    }
    Ok(())
}

/// Refuse service manifest tags that duplicate or disagree with `contract`.
///
/// Every lane transport carries one `oa:transport:` tag and no others.
///
/// # Errors
///
/// Returns [`RefusalCode::IdentityMismatch`] when a tag names another
/// profile or an undeclared transport.
pub fn check_service_tags(tags: &[Tag], contract: &ServiceContract) -> Result<(), ContractError> {
    let mut declared: Vec<&str> = contract
        .lanes
        .iter()
        .map(|lane| lane.transport.as_str())
        .collect();
    declared.sort_unstable();
    declared.dedup();
    let mut marker = 0;
    let mut profile = 0;
    let mut seen: Vec<&str> = Vec::new();
    for tag in tags {
        if tag.name() != Some("t") {
            continue;
        }
        let value = tag.value().unwrap_or("");
        if value == CAP_MARKER {
            marker += 1;
        } else if value == Profile::Service.tag() {
            profile += 1;
        } else if let Some(lane) = value.strip_prefix("oa:transport:") {
            if !declared.contains(&lane) || seen.contains(&lane) {
                return Err(ContractError::new(
                    RefusalCode::IdentityMismatch,
                    "discovery tag",
                ));
            }
            seen.push(lane);
        } else if value.starts_with("oa:profile:") {
            return Err(ContractError::new(
                RefusalCode::IdentityMismatch,
                "discovery tag",
            ));
        }
    }
    if marker != 1 || profile != 1 {
        return Err(malformed("discovery tags"));
    }
    seen.sort_unstable();
    if seen != declared {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "lane transport tag",
        ));
    }
    Ok(())
}

/// One transport lane a service answers on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceLane {
    /// `http` or `nostr-cj`.
    pub transport: String,
    /// HTTP lane: public base URL.
    pub endpoint: Option<String>,
    /// HTTP lane: decision call path.
    pub call: Option<String>,
    /// HTTP lane: caller's model discovery path.
    pub models: Option<String>,
    /// NIP-CJ lane: worker pubkey that answers.
    pub worker: Option<String>,
    /// NIP-CJ lane: relays that carry the job family.
    pub relays: Vec<String>,
    /// NIP-CJ lane: job request kind.
    pub request_kind: Option<u16>,
    /// NIP-CJ lane: job result kind.
    pub result_kind: Option<u16>,
    /// NIP-CJ lane: job feedback kind.
    pub feedback_kind: Option<u16>,
}

/// A door an unauthenticated caller may name. Tenant bindings never appear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDoor {
    /// The door name a request carries.
    pub name: String,
    /// The model the door serves.
    pub model: String,
    /// The bound artifact identity, `sha256:<digest>`.
    pub artifact_signature: String,
}

/// The `service` object of a `service`-profile binding contract: what the
/// serving path claims to speak and serve. A claim checked at use, not
/// evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceContract {
    /// The request schema version, matching `interface`.
    pub interface: String,
    /// Transports the service answers on.
    pub lanes: Vec<ServiceLane>,
    /// Anonymous-reachable doors only.
    pub doors: Vec<ServiceDoor>,
    /// Lane-level ceilings the service enforces.
    pub limits: BTreeMap<String, u64>,
    /// Named schema versions: `request` and `receipt`.
    pub versions: BTreeMap<String, String>,
}

impl ServiceContract {
    fn transport_name(&self) -> String {
        "service".into()
    }

    /// The door named `name`, if the manifest advertises it.
    #[must_use]
    pub fn door(&self, name: &str) -> Option<&ServiceDoor> {
        self.doors.iter().find(|door| door.name == name)
    }

    /// The first `nostr-cj` lane, if the service answers on the relay.
    #[must_use]
    pub fn relay_lane(&self) -> Option<&ServiceLane> {
        self.lanes.iter().find(|lane| lane.transport == "nostr-cj")
    }
}

/// An operator-provisioned discovery pin: the publisher and slug a client
/// trusts. A signed `30180` alone is not this trust.
#[derive(Debug, Clone)]
pub struct ServiceTrust<'a> {
    /// Expected publisher pubkey, 64 lowercase hex.
    pub publisher: &'a str,
    /// The `d` slug the manifest answers to.
    pub slug: &'a str,
    /// How old a manifest may be before it is stale.
    pub max_age_seconds: u64,
}

/// A resolved service manifest bound to the event that carried it.
#[derive(Debug, Clone)]
pub struct ServiceManifest {
    /// The manifest event's own id.
    pub event_id: String,
    /// The signing publisher.
    pub publisher: String,
    /// The manifest's `created_at`.
    pub created_at: u64,
    /// The portable definition.
    pub definition: Definition,
    /// The parsed service contract.
    pub contract: ServiceContract,
}

const SERVICE_FUTURE_SKEW_SECONDS: u64 = 300;

/// Parse the `binding_contract` of a `service`-profile definition.
///
/// # Errors
///
/// Returns a typed refusal for an unknown field, lane, door, or version
/// disagreement.
pub fn parse_service_contract(
    contract: &Map<String, Value>,
) -> Result<ServiceContract, ContractError> {
    reject(contract, &["interface", "service"], "binding_contract")?;
    let interface = text(
        require(contract, "interface", "binding_contract")?,
        "interface",
    )?
    .to_string();
    if interface.is_empty() {
        return Err(malformed("interface"));
    }
    let service = as_map(require(contract, "service", "binding_contract")?, "service")?;
    reject(
        service,
        &["lanes", "doors", "limits", "versions"],
        "service",
    )?;
    let versions = as_map(require(service, "versions", "service")?, "versions")?;
    reject(versions, &["request", "receipt"], "versions")?;
    let request_version = text(
        require(versions, "request", "versions")?,
        "versions.request",
    )?;
    if request_version != interface {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "versions.request",
        ));
    }
    let receipt_version = text(
        require(versions, "receipt", "versions")?,
        "versions.receipt",
    )?;
    let lanes = require(service, "lanes", "service")?
        .as_array()
        .ok_or_else(|| malformed("lanes"))?;
    if lanes.is_empty() || lanes.len() > 8 {
        return Err(malformed("lanes"));
    }
    let lanes = lanes
        .iter()
        .map(|lane| service_lane(lane, &interface))
        .collect::<Result<Vec<_>, _>>()?;
    let doors = require(service, "doors", "service")?
        .as_array()
        .ok_or_else(|| malformed("doors"))?;
    if doors.len() > 256 {
        return Err(malformed("doors"));
    }
    let doors = doors
        .iter()
        .map(service_door)
        .collect::<Result<Vec<_>, _>>()?;
    let limits_object = as_map(require(service, "limits", "service")?, "limits")?;
    if limits_object.len() > 64 {
        return Err(malformed("limits"));
    }
    let mut limits = BTreeMap::new();
    for (name, ceiling) in limits_object {
        let ceiling = ceiling.as_u64().ok_or_else(|| malformed("limits"))?;
        limits.insert(name.clone(), ceiling);
    }
    let mut versions_map = BTreeMap::new();
    versions_map.insert("request".to_string(), request_version.to_string());
    versions_map.insert("receipt".to_string(), receipt_version.to_string());
    Ok(ServiceContract {
        interface,
        lanes,
        doors,
        limits,
        versions: versions_map,
    })
}

/// Resolve a service manifest from discovery events under an operator pin.
///
/// The newest cryptographically valid event matching `(publisher, kind,
/// slug)` is the advertisement. A stale, malformed, or disagreeing newest
/// event refuses rather than falling back to an older one: an older answer
/// could mask a poisoning.
///
/// # Errors
///
/// Returns [`RefusalCode::Unavailable`] when no event matches the pin,
/// [`RefusalCode::Stale`] when the newest match is outside the freshness
/// window or expired, and the body's own refusal otherwise.
pub fn resolve_service<'a>(
    events: impl IntoIterator<Item = &'a Event>,
    trust: &ServiceTrust,
    now: u64,
) -> Result<ServiceManifest, ContractError> {
    if !is_hex(trust.publisher) {
        return Err(malformed("trust publisher"));
    }
    if !is_slug(trust.slug) {
        return Err(malformed("trust slug"));
    }
    let candidate = events
        .into_iter()
        .filter(|event| {
            event.kind == DISCOVERY_KIND
                && event.pubkey == trust.publisher
                && event.tag_values("d").any(|value| value == trust.slug)
        })
        .max_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
    let Some(event) = candidate else {
        return Err(ContractError::new(
            RefusalCode::Unavailable,
            "no service manifest for the pin",
        ));
    };
    event
        .validate_structure()
        .and_then(|()| event.validate_crypto())
        .map_err(|_| ContractError::new(RefusalCode::Malformed, "manifest event"))?;
    if event.tag_values("d").count() != 1 {
        return Err(malformed("manifest d tag"));
    }
    if event.is_expired(now)
        || event.created_at > now.saturating_add(SERVICE_FUTURE_SKEW_SECONDS)
        || now.saturating_sub(event.created_at) > trust.max_age_seconds
    {
        return Err(ContractError::new(RefusalCode::Stale, "service manifest"));
    }
    let body = parse_strict(event.content.as_bytes())?;
    let root = as_map(&body, "manifest")?;
    reject(root, &["definition"], "manifest")?;
    let definition = parse_definition(require(root, "definition", "manifest")?)?;
    if definition.profile != Profile::Service {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "profile",
        ));
    }
    if definition.id.split(':').next() != Some(event.pubkey.as_str()) {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "definition id publisher",
        ));
    }
    let contract = as_map(
        require(
            require(root, "definition", "manifest")?
                .as_object()
                .ok_or_else(|| malformed("definition"))?,
            "binding_contract",
            "definition",
        )?,
        "binding_contract",
    )?;
    let contract = parse_service_contract(contract)?;
    check_service_tags(&event.tags, &contract)?;
    Ok(ServiceManifest {
        event_id: event.id.clone(),
        publisher: event.pubkey.clone(),
        created_at: event.created_at,
        definition,
        contract,
    })
}

fn service_lane(lane: &Value, interface: &str) -> Result<ServiceLane, ContractError> {
    let lane = as_map(lane, "lane")?;
    let transport = enum_text(
        require(lane, "transport", "lane")?,
        &["http", "nostr-cj"],
        "lane.transport",
    )?;
    match transport.as_str() {
        "http" => {
            reject(lane, &["transport", "endpoint", "call", "models"], "lane")?;
            let endpoint = text(require(lane, "endpoint", "lane")?, "endpoint")?;
            if !endpoint.starts_with("https://") && !endpoint.starts_with("http://") {
                return Err(malformed("endpoint"));
            }
            let call = text(require(lane, "call", "lane")?, "call")?;
            let models = text(require(lane, "models", "lane")?, "models")?;
            if !call.starts_with('/') || !models.starts_with('/') {
                return Err(malformed("lane paths"));
            }
            Ok(ServiceLane {
                transport,
                endpoint: Some(endpoint.to_string()),
                call: Some(call.to_string()),
                models: Some(models.to_string()),
                worker: None,
                relays: Vec::new(),
                request_kind: None,
                result_kind: None,
                feedback_kind: None,
            })
        }
        _ => {
            reject(
                lane,
                &[
                    "transport",
                    "worker",
                    "relays",
                    "request_kind",
                    "result_kind",
                    "feedback_kind",
                ],
                "lane",
            )?;
            let worker = hex_key(text(require(lane, "worker", "lane")?, "worker")?)?;
            let relays = string_list(require(lane, "relays", "lane")?, "relays")?;
            if relays.is_empty() || relays.len() > 8 {
                return Err(malformed("relays"));
            }
            for relay in &relays {
                if !relay.starts_with("ws://") && !relay.starts_with("wss://") {
                    return Err(malformed("relays"));
                }
            }
            let request_kind = lane_kind(lane, "request_kind")?;
            let result_kind = lane_kind(lane, "result_kind")?;
            let feedback_kind = lane_kind(lane, "feedback_kind")?;
            if interface == crate::decision::SCHEMA
                && (request_kind != crate::decision::REQUEST_KIND
                    || result_kind != crate::decision::RESULT_KIND
                    || feedback_kind != crate::decision::FEEDBACK_KIND)
            {
                return Err(ContractError::new(RefusalCode::Incompatible, "lane kinds"));
            }
            Ok(ServiceLane {
                transport,
                endpoint: None,
                call: None,
                models: None,
                worker: Some(worker),
                relays,
                request_kind: Some(request_kind),
                result_kind: Some(result_kind),
                feedback_kind: Some(feedback_kind),
            })
        }
    }
}

fn lane_kind(lane: &Map<String, Value>, key: &str) -> Result<u16, ContractError> {
    let value = require(lane, key, "lane")?
        .as_u64()
        .ok_or_else(|| malformed(format!("lane.{key}")))?;
    u16::try_from(value).map_err(|_| malformed(format!("lane.{key}")))
}

fn service_door(door: &Value) -> Result<ServiceDoor, ContractError> {
    let door = as_map(door, "door")?;
    reject(door, &["name", "model", "artifact_signature"], "door")?;
    let name = slug(require(door, "name", "door")?)?;
    let model = text(require(door, "model", "door")?, "model")?;
    if model.is_empty() || model.len() > 128 {
        return Err(malformed("model"));
    }
    let signature = text(
        require(door, "artifact_signature", "door")?,
        "artifact_signature",
    )?;
    let Some(digest) = signature.strip_prefix("sha256:") else {
        return Err(malformed("artifact_signature"));
    };
    if !is_hex(digest) {
        return Err(malformed("artifact_signature"));
    }
    Ok(ServiceDoor {
        name,
        model: model.to_string(),
        artifact_signature: signature.to_string(),
    })
}

/// Parse an operator preference. `prefer` does not admit a denied id.
///
/// # Errors
///
/// Returns a typed refusal when the body is not a preference.
pub fn parse_preference(value: &Value) -> Result<Preference, ContractError> {
    let object = as_map(value, "preference")?;
    reject(
        object,
        &[
            "v",
            "requires",
            "prefer",
            "deny",
            "ceilings",
            "assurance",
            "disclosure_policy",
            "meta",
        ],
        "preference",
    )?;
    version(object, "preference")?;
    let prefer = qualified_list(require(object, "prefer", "preference")?)?;
    let deny = qualified_list(require(object, "deny", "preference")?)?;
    if prefer.iter().any(|id| deny.contains(id)) {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "preference prefers a denied id",
        ));
    }
    let assurance = enum_text(
        require(object, "assurance", "preference")?,
        &["host_enforced", "trusted_contract"],
        "assurance",
    )?;
    let _policy = parse_artifact(require(object, "disclosure_policy", "preference")?)?;
    let _ceilings = as_map(require(object, "ceilings", "preference")?, "ceilings")?;
    Ok(Preference {
        prefer,
        deny,
        assurance,
    })
}

/// Parse a host grant. Callers keep this outside the definition document.
///
/// # Errors
///
/// Returns a typed refusal when the body is not a grant.
pub fn parse_grant(value: &Value) -> Result<Grant, ContractError> {
    let object = as_map(value, "grant")?;
    reject(
        object,
        &[
            "v",
            "requires",
            "principal",
            "binding",
            "scope",
            "budget",
            "meta",
        ],
        "grant",
    )?;
    version(object, "grant")?;
    let principal = hex_key(text(require(object, "principal", "grant")?, "principal")?)?;
    let binding = qualified(require(object, "binding", "grant")?)?;
    let scope = string_list(require(object, "scope", "grant")?, "scope")?;
    let _budget = as_map(require(object, "budget", "grant")?, "budget")?;
    Ok(Grant {
        principal,
        binding,
        scope,
    })
}

/// Whether a kind-30181 event may be shown to `readers`.
///
/// A private policy is visible only to its signer. Any other reader, including
/// one that knows the event id, sees nothing. A public policy stays readable.
#[must_use]
pub fn private_policy_visible(event: &Event, readers: &std::collections::HashSet<String>) -> bool {
    if event.kind != PREFERENCE_KIND {
        return true;
    }
    let private = event
        .tags
        .iter()
        .any(|tag| tag.name() == Some("t") && tag.value() == Some(PRIVATE_POLICY_MARKER));
    if !private {
        return true;
    }
    let recipient_ok = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("p"))
        .count()
        == 1
        && event
            .tags
            .iter()
            .any(|tag| tag.name() == Some("p") && tag.value() == Some(event.pubkey.as_str()));
    let mailbox_ok = event
        .tags
        .iter()
        .any(|tag| tag.name() == Some("d") && tag.value().is_some_and(is_random_mailbox));
    recipient_ok && mailbox_ok && readers.contains(&event.pubkey)
}

/// Refuse a private preference whose tags would publish a machine name.
///
/// # Errors
///
/// Returns [`RefusalCode::Malformed`] when `d` is not a random mailbox, `p`
/// is not the signer, or the content is plaintext JSON.
pub fn check_private_policy(event: &Event) -> Result<(), ContractError> {
    if event.kind != PREFERENCE_KIND {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "preference kind",
        ));
    }
    if !private_policy_visible(
        event,
        &std::collections::HashSet::from([event.pubkey.clone()]),
    ) {
        return Err(malformed("private preference tags"));
    }
    if event.content.trim_start().starts_with('{') {
        return Err(malformed("private preference content is plaintext"));
    }
    Ok(())
}

fn binding_contract(
    profile: Profile,
    contract: &Map<String, Value>,
) -> Result<(String, Vec<String>), ContractError> {
    match profile {
        Profile::Native => {
            reject(contract, &["operation", "interface"], "binding_contract")?;
            let _operation = slug(require(contract, "operation", "binding_contract")?)?;
            let _interface = text(
                require(contract, "interface", "binding_contract")?,
                "interface",
            )?;
            Ok(("native".into(), Vec::new()))
        }
        Profile::Executor => {
            reject(
                contract,
                &[
                    "interface",
                    "transport",
                    "task",
                    "context",
                    "isolation",
                    "remote",
                ],
                "binding_contract",
            )?;
            let transport = enum_text(
                require(contract, "transport", "binding_contract")?,
                &["subprocess", "acp", "http", "nostr-cj"],
                "transport",
            )?;
            let _interface = text(
                require(contract, "interface", "binding_contract")?,
                "interface",
            )?;
            let _task = parse_schema(require(contract, "task", "binding_contract")?)?;
            let _context = parse_schema(require(contract, "context", "binding_contract")?)?;
            let isolation = string_list(
                require(contract, "isolation", "binding_contract")?,
                "isolation",
            )?;
            check_remote(contract, &transport)?;
            Ok((transport, isolation))
        }
        Profile::Plugin => {
            reject(contract, &["plugin", "abi"], "binding_contract")?;
            let _plugin = qualified(require(contract, "plugin", "binding_contract")?)?;
            let _abi = string_list(require(contract, "abi", "binding_contract")?, "abi")?;
            Ok(("plugin".into(), Vec::new()))
        }
        Profile::Adapter => {
            reject(
                contract,
                &["interface", "transport", "operations", "remote"],
                "binding_contract",
            )?;
            let transport = enum_text(
                require(contract, "transport", "binding_contract")?,
                &["mcp", "http", "subprocess", "nostr-cj"],
                "transport",
            )?;
            let _interface = text(
                require(contract, "interface", "binding_contract")?,
                "interface",
            )?;
            let _operations = string_list(
                require(contract, "operations", "binding_contract")?,
                "operations",
            )?;
            check_remote(contract, &transport)?;
            Ok((transport, Vec::new()))
        }
        Profile::Service => {
            let contract = parse_service_contract(contract)?;
            Ok((contract.transport_name(), Vec::new()))
        }
    }
}

fn check_remote(contract: &Map<String, Value>, transport: &str) -> Result<(), ContractError> {
    let Some(remote) = contract.get("remote") else {
        if transport == "nostr-cj" || transport == "http" {
            return Err(malformed("remote hint"));
        }
        return Ok(());
    };
    let remote = as_map(remote, "remote")?;
    reject(
        remote,
        &["worker", "relays", "endpoint", "identity"],
        "remote",
    )?;
    if transport == "nostr-cj" {
        hex_key(text(require(remote, "worker", "remote")?, "worker")?)?;
        let relays = string_list(require(remote, "relays", "remote")?, "relays")?;
        if relays.is_empty() || relays.len() > 8 {
            return Err(malformed("relays"));
        }
        for relay in &relays {
            if !relay.starts_with("ws://") && !relay.starts_with("wss://") {
                return Err(malformed("relays"));
            }
        }
    }
    if transport == "http" {
        let endpoint = text(require(remote, "endpoint", "remote")?, "endpoint")?;
        if !endpoint.starts_with("https://") && !endpoint.starts_with("http://") {
            return Err(malformed("endpoint"));
        }
    }
    Ok(())
}

fn support_bounds(value: &Value) -> Result<BTreeMap<String, Support>, ContractError> {
    let object = as_map(value, "bounds")?;
    let mut bounds = BTreeMap::new();
    for (name, level) in object {
        if !is_bound(name) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "support bound",
            ));
        }
        let level = match text(level, "bounds")? {
            "enforced" => Support::Enforced,
            "not_enforced" => Support::NotEnforced,
            "unknown" => Support::Unknown,
            _ => {
                return Err(ContractError::new(
                    RefusalCode::UnsupportedFeature,
                    "support bound",
                ));
            }
        };
        bounds.insert(name.clone(), level);
    }
    Ok(bounds)
}

fn is_bound(name: &str) -> bool {
    matches!(
        name,
        "wall_ms"
            | "memory_bytes"
            | "input_bytes"
            | "output_bytes"
            | "read_bytes"
            | "storage_bytes"
            | "calls"
            | "attempts"
            | "concurrency"
            | "depth"
            | "fuel"
            | "spend_microunits"
    )
}

fn minimum(value: &Value) -> Result<BTreeMap<String, u64>, ContractError> {
    let object = as_map(value, "minimum")?;
    let mut minimum = BTreeMap::new();
    for (name, ceiling) in object {
        if !is_bound(name) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "minimum bound",
            ));
        }
        let ceiling = ceiling.as_u64().ok_or_else(|| malformed("minimum"))?;
        minimum.insert(name.clone(), ceiling);
    }
    Ok(minimum)
}

fn parse_schema(value: &Value) -> Result<ArtifactRef, ContractError> {
    let artifact = parse_artifact(value)?;
    if artifact.media_type != "application/schema+json" {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "schema media type",
        ));
    }
    Ok(artifact)
}

fn version(object: &Map<String, Value>, path: &str) -> Result<(), ContractError> {
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            format!("{path}.v"),
        ));
    }
    let requires = require(object, "requires", path)?
        .as_array()
        .ok_or_else(|| malformed(format!("{path}.requires")))?;
    if !requires.is_empty() {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            format!("{path}.requires"),
        ));
    }
    if let Some(meta) = object.get("meta") {
        meta.as_object()
            .ok_or_else(|| malformed(format!("{path}.meta")))?;
    }
    Ok(())
}

fn as_map<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ContractError> {
    value.as_object().ok_or_else(|| malformed(path))
}

fn require<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Value, ContractError> {
    object
        .get(key)
        .ok_or_else(|| malformed(format!("{path}.{key}")))
}

fn reject(object: &Map<String, Value>, allowed: &[&str], path: &str) -> Result<(), ContractError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                format!("{path}.{key}"),
            ));
        }
    }
    Ok(())
}

fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str, ContractError> {
    value.as_str().ok_or_else(|| malformed(path))
}

fn enum_text(value: &Value, allowed: &[&str], path: &str) -> Result<String, ContractError> {
    let text = text(value, path)?;
    if !allowed.contains(&text) {
        return Err(ContractError::new(RefusalCode::UnsupportedFeature, path));
    }
    Ok(text.to_string())
}

fn string_list(value: &Value, path: &str) -> Result<Vec<String>, ContractError> {
    let array = value.as_array().ok_or_else(|| malformed(path))?;
    array
        .iter()
        .map(|item| text(item, path).map(str::to_string))
        .collect()
}

fn qualified_list(value: &Value) -> Result<Vec<String>, ContractError> {
    let array = value.as_array().ok_or_else(|| malformed("ids"))?;
    array.iter().map(qualified).collect()
}

fn qualified(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "id")?;
    let Some((publisher, rest)) = text.split_once(':') else {
        return Err(malformed("id"));
    };
    let Some((package, component)) = rest.split_once('/') else {
        return Err(malformed("id"));
    };
    if !is_hex(publisher) || !is_slug(package) || !is_slug(component) || component.contains('/') {
        return Err(malformed("id"));
    }
    Ok(text.to_string())
}

fn slug(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "slug")?;
    if !is_slug(text) {
        return Err(malformed("slug"));
    }
    Ok(text.to_string())
}

fn is_slug(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    value.len() <= 64
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

fn hex_key(value: &str) -> Result<String, ContractError> {
    if !is_hex(value) {
        return Err(malformed("pubkey"));
    }
    Ok(value.to_string())
}

fn is_hex(value: &str) -> bool {
    value.len() == PUBLISHER_HEX
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_random_mailbox(value: &str) -> bool {
    is_hex(value)
}

fn malformed(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::Malformed, detail)
}

/// Parse a JSON object with the shared strict parser. Exposed for callers
/// that already hold bytes.
///
/// # Errors
///
/// Returns the strict parser's refusal.
pub fn parse_json(bytes: &[u8]) -> Result<Value, ContractError> {
    parse_strict(bytes)
}

impl BoundAssignment {
    fn bound_name(&self) -> &'static str {
        match self.bound {
            crate::contracts::Bound::WallMs => "wall_ms",
            crate::contracts::Bound::MemoryBytes => "memory_bytes",
            crate::contracts::Bound::InputBytes => "input_bytes",
            crate::contracts::Bound::OutputBytes => "output_bytes",
            crate::contracts::Bound::ReadBytes => "read_bytes",
            crate::contracts::Bound::StorageBytes => "storage_bytes",
            crate::contracts::Bound::Calls => "calls",
            crate::contracts::Bound::Attempts => "attempts",
            crate::contracts::Bound::Concurrency => "concurrency",
            crate::contracts::Bound::Depth => "depth",
            crate::contracts::Bound::Fuel => "fuel",
            crate::contracts::Bound::SpendMicrounits => "spend_microunits",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PUB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SCHEMA: &str = "sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0";

    fn schema() -> Value {
        json!({"digest": SCHEMA, "size": 17, "media_type": "application/schema+json"})
    }

    fn effects() -> Value {
        json!({"reads": ["workspace"], "writes": [], "network": [], "process": false, "delegates": false, "spend": false})
    }

    fn definition(profile: &str, contract: Value) -> Value {
        json!({
            "v": 1,
            "requires": [],
            "id": format!("{PUB}:openagents/demo"),
            "profile": profile,
            "summary": "a portable interface",
            "input": schema(),
            "output": schema(),
            "effects": effects(),
            "minimum": {},
            "support": {
                "bounds": {"wall_ms": "unknown"},
                "cancellation": "unsupported",
                "idempotency": "none",
                "evidence": []
            },
            "binding_contract": contract
        })
    }

    #[test]
    fn four_profiles_parse_and_old_executor_fields_do_not() {
        let native = definition(
            "native",
            json!({"operation": "mine", "interface": "host.v1"}),
        );
        assert_eq!(parse_definition(&native).unwrap().profile, Profile::Native);
        let executor = definition(
            "executor",
            json!({"interface": "task.v1", "transport": "subprocess", "task": schema(), "context": schema(), "isolation": ["directory"]}),
        );
        assert_eq!(parse_definition(&executor).unwrap().transport, "subprocess");
        let plugin = definition(
            "plugin",
            json!({"plugin": format!("{PUB}:openagents/guest"), "abi": ["openagents.plugin-packet.v1"]}),
        );
        assert_eq!(parse_definition(&plugin).unwrap().profile, Profile::Plugin);
        let adapter = definition(
            "adapter",
            json!({"interface": "project.v1", "transport": "subprocess", "operations": ["list"]}),
        );
        assert_eq!(
            parse_definition(&adapter).unwrap().profile,
            Profile::Adapter
        );

        let mut legacy = executor.clone();
        legacy["enforces"] = json!(["minutes"]);
        assert_eq!(
            parse_definition(&legacy).unwrap_err().code,
            RefusalCode::UnsupportedFeature
        );
    }

    #[test]
    fn an_omitted_bound_is_unknown_and_a_claim_does_not_cover_a_minimum() {
        let mut body = definition(
            "executor",
            json!({"interface": "task.v1", "transport": "subprocess", "task": schema(), "context": schema(), "isolation": []}),
        );
        let parsed = parse_definition(&body).unwrap();
        assert_eq!(bound_support(&parsed, "calls"), Support::Unknown);
        body["minimum"] = json!({"wall_ms": 1000});
        body["support"]["bounds"] = json!({"wall_ms": "not_enforced"});
        let parsed = parse_definition(&body).unwrap();
        let claim = crate::contracts::parse_bound(&json!({
            "bound": "wall_ms",
            "ceiling": 1000,
            "assurance": "trusted_executor",
            "mechanism": "executor"
        }))
        .unwrap();
        assert_eq!(
            cover(&parsed, &[claim]).unwrap_err().code,
            RefusalCode::CannotEnforce
        );
        let host = crate::contracts::parse_bound(&json!({
            "bound": "wall_ms",
            "ceiling": 1000,
            "assurance": "host",
            "mechanism": "supervisor"
        }))
        .unwrap();
        assert!(cover(&parsed, &[host]).is_ok());
    }

    #[test]
    fn discovery_tags_must_agree_and_private_policy_hides_machine_names() {
        let parsed = parse_definition(&definition(
            "native",
            json!({"operation": "mine", "interface": "host.v1"}),
        ))
        .unwrap();
        let tags = discovery_tags(&parsed);
        check_discovery_tags(&tags, &parsed).unwrap();
        let mut disagreed = tags.clone();
        disagreed.push(Tag::new(vec!["t".into(), "oa:profile:executor".into()]));
        assert_eq!(
            check_discovery_tags(&disagreed, &parsed).unwrap_err().code,
            RefusalCode::IdentityMismatch
        );

        let preference = json!({
            "v": 1,
            "requires": [],
            "prefer": [format!("{PUB}:openagents/demo")],
            "deny": [],
            "ceilings": {},
            "assurance": "host_enforced",
            "disclosure_policy": {"digest": SCHEMA, "size": 17, "media_type": "text/plain"}
        });
        assert!(parse_preference(&preference).is_ok());

        let signer = crate::domain::RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let event = signer.sign(
            1,
            PREFERENCE_KIND,
            vec![
                Tag::new(vec!["t".into(), PRIVATE_POLICY_MARKER.into()]),
                Tag::new(vec!["p".into(), signer.pubkey().into()]),
                Tag::new(vec!["d".into(), "ab".repeat(32)]),
            ],
            "bm90LWpzb24=".into(),
        );
        check_private_policy(&event).unwrap();
        assert!(!private_policy_visible(
            &event,
            &std::collections::HashSet::new()
        ));
        let mut named = event.clone();
        named.tags[2] = Tag::new(vec!["d".into(), "laptop.local".into()]);
        assert!(check_private_policy(&named).is_err());
    }

    const WORKER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ARTIFACT: &str =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn service_contract() -> Value {
        json!({
            "interface": "openagents.systemone.v1",
            "service": {
                "lanes": [
                    {
                        "transport": "http",
                        "endpoint": "https://gateway.example",
                        "call": "/v1/systemone",
                        "models": "/v1/models"
                    },
                    {
                        "transport": "nostr-cj",
                        "worker": WORKER,
                        "relays": ["wss://relay.example"],
                        "request_kind": 25910,
                        "result_kind": 26910,
                        "feedback_kind": 27010
                    }
                ],
                "doors": [
                    {"name": "shared-kev", "model": "kev-0.6b", "artifact_signature": ARTIFACT}
                ],
                "limits": {"max_questions": 64, "request_window_seconds": 600},
                "versions": {
                    "request": "openagents.systemone.v1",
                    "receipt": "openagents.receipt.execution.v1"
                }
            }
        })
    }

    fn service_event(signer: &crate::domain::RelaySigner, at: u64) -> Event {
        let mut body = definition("service", service_contract());
        body["id"] = json!(format!("{}:openagents/decision-edge", signer.pubkey()));
        let contract =
            parse_service_contract(body["binding_contract"].as_object().unwrap()).unwrap();
        let mut tags = vec![Tag::new(vec!["d".into(), "decision-edge".into()])];
        tags.extend(service_tags(&contract));
        signer.sign(
            at,
            DISCOVERY_KIND,
            tags,
            json!({"definition": body}).to_string(),
        )
    }

    #[test]
    fn service_definition_parses_and_tags_agree() {
        let parsed = parse_definition(&definition("service", service_contract())).unwrap();
        assert_eq!(parsed.profile, Profile::Service);
        let contract = parse_service_contract(
            &definition("service", service_contract())["binding_contract"]
                .as_object()
                .unwrap()
                .clone(),
        )
        .unwrap();
        assert_eq!(contract.lanes.len(), 2);
        assert_eq!(
            contract.relay_lane().unwrap().worker.as_deref(),
            Some(WORKER)
        );
        assert_eq!(contract.door("shared-kev").unwrap().model, "kev-0.6b");
        assert!(contract.door("acme-kev").is_none());
        check_service_tags(&service_tags(&contract), &contract).unwrap();
        check_discovery_tags(&service_tags(&contract), &parsed).unwrap();

        let mut wrong = service_contract();
        wrong["service"]["lanes"][1]["request_kind"] = json!(25900);
        assert_eq!(
            parse_service_contract(wrong.as_object().unwrap())
                .unwrap_err()
                .code,
            RefusalCode::Incompatible
        );
        let mut disagreed = service_contract();
        disagreed["service"]["versions"]["request"] = json!("openagents.systemone.v2");
        assert_eq!(
            parse_service_contract(disagreed.as_object().unwrap())
                .unwrap_err()
                .code,
            RefusalCode::Incompatible
        );
    }

    #[test]
    fn resolve_service_trusts_the_pin_and_refuses_stale_or_absent() {
        let signer = crate::domain::RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        let trust = ServiceTrust {
            publisher: signer.pubkey(),
            slug: "decision-edge",
            max_age_seconds: 600,
        };
        let now = 1_000_000;
        let manifest =
            resolve_service([service_event(&signer, now - 30)].iter(), &trust, now).unwrap();
        assert_eq!(
            manifest.contract.door("shared-kev").unwrap().model,
            "kev-0.6b"
        );

        assert_eq!(
            resolve_service(Vec::<&Event>::new(), &trust, now)
                .unwrap_err()
                .code,
            RefusalCode::Unavailable
        );
        assert_eq!(
            resolve_service([service_event(&signer, now - 601)].iter(), &trust, now)
                .unwrap_err()
                .code,
            RefusalCode::Stale
        );
        let wrong_signer = crate::domain::RelaySigner::from_secret_hex(&"33".repeat(32)).unwrap();
        assert_eq!(
            resolve_service([service_event(&wrong_signer, now)].iter(), &trust, now)
                .unwrap_err()
                .code,
            RefusalCode::Unavailable
        );
    }

    #[test]
    fn resolve_service_picks_newest_and_refuses_a_bad_newest() {
        let signer = crate::domain::RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        let trust = ServiceTrust {
            publisher: signer.pubkey(),
            slug: "decision-edge",
            max_age_seconds: 600,
        };
        let now = 1_000_000;
        let old = service_event(&signer, now - 100);
        let new = service_event(&signer, now - 10);
        let manifest = resolve_service([&old, &new], &trust, now).unwrap();
        assert_eq!(manifest.event_id, new.id);

        let mut unsupported = definition("service", service_contract());
        unsupported["id"] = json!(format!("{}:openagents/decision-edge", signer.pubkey()));
        unsupported["v"] = json!(99);
        let contract = parse_service_contract(service_contract().as_object().unwrap()).unwrap();
        let mut tags = vec![Tag::new(vec!["d".into(), "decision-edge".into()])];
        tags.extend(service_tags(&contract));
        let bad = signer.sign(
            now,
            DISCOVERY_KIND,
            tags,
            json!({"definition": unsupported}).to_string(),
        );
        assert_eq!(
            resolve_service([&old, &bad], &trust, now).unwrap_err().code,
            RefusalCode::UnsupportedVersion
        );
    }
}
