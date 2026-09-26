//! Check stored indexes against their original signed declarations on reopen.
use super::*;

pub(super) fn validate(document: &Document, secret: &SecretKey) -> Result<()> {
    for (digest, invitation) in &document.invitations {
        signed(
            &invitation.event,
            &invitation.body,
            &document.setup.authority,
            secret,
        )?;
        if &nostr::contracts::digest_bytes(&jcs(&invitation.body).map_err(|e| e.to_string())?)
            != digest
            || invitation.body["scope"] != document.setup.scope
            || invitation.body["policy"] != document.setup.policy
        {
            return Err("retained invitation index differs from its original declaration".into());
        }
    }
    for (digest, access) in &document.grants {
        signed(
            &access.event,
            &access.body,
            &document.setup.authority,
            secret,
        )?;
        if &nostr::contracts::digest_bytes(&jcs(&access.body).map_err(|e| e.to_string())?) != digest
            || access.body["scope"] != document.setup.scope
            || access.body["policy"] != document.setup.policy
        {
            return Err("retained grant index differs from its original declaration".into());
        }
        if let Some(revoked) = &access.revoked {
            control::validate(revoked).map_err(|e| e.to_string())?;
            if revoked["authority"] != document.setup.authority
                || super::digest(&revoked["grant"])? != *digest
            {
                return Err("retained revocation differs from its grant".into());
            }
        }
    }
    for (key, record) in &document.commands {
        let opened =
            nostr::private_artifact::open(&record.input, secret).map_err(|e| e.to_string())?;
        let bytes = opened
            .inline_bytes()
            .ok_or("retained command bytes unavailable")?;
        let body = control::parse(bytes).map_err(|e| e.to_string())?;
        if body["v"] != control::COMMAND
            || record.fingerprint != nostr::contracts::digest_bytes(bytes)
            || *key
                != format!(
                    "{}|{}",
                    digest(&body["grant"])?,
                    body["command"].as_str().ok_or("retained command id")?
                )
        {
            return Err("retained command fingerprint or index differs".into());
        }
        let grant = document
            .grants
            .get(&digest(&body["grant"])?)
            .ok_or("retained command grant missing")?;
        if grant.body["client"] != opened.signer() || body["scope"] != document.setup.scope {
            return Err("retained command principal or scope differs".into());
        }
        let local = coder::task::parse_command(&record.local).map_err(|e| e.to_string())?;
        if local.task_id != document.setup.task_id
            || local.expected_revision != body["expected_revision"].as_u64()
            || local.command_id
                != format!(
                    "ctrl-{}",
                    nostr::contracts::digest_bytes(key.as_bytes()).trim_start_matches("sha256:")
                )
        {
            return Err("retained local command binding differs".into());
        }
        match &local.action {
            coder::task::Action::Correct { prompt, .. } if body["action"] == "steer" => {
                nostr::contracts::check_artifact_bytes(
                    &parse_artifact(&body["payload"]["message"]).map_err(|e| e.to_string())?,
                    prompt.as_bytes(),
                )
                .map_err(|e| e.to_string())?;
            }
            coder::task::Action::Cancel { reason } if body["action"] == "cancel" => {
                let expected = body["payload"]["reason"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("Scoped Nostr client cancellation");
                if reason != expected {
                    return Err("retained cancellation text differs".into());
                }
            }
            _ => return Err("retained local command acquired another action".into()),
        }
        if let Some(answer) = &record.answer {
            control::validate(answer).map_err(|e| e.to_string())?;
            if answer["v"] != control::COMMAND_RESULT
                || digest(&answer["command"])? != record.fingerprint
            {
                return Err("retained command answer differs".into());
            }
        }
    }
    for (key, job) in &document.jobs {
        let opened = execution::open_request(
            &job.request,
            &document.setup.authority,
            secret,
            job.request.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = opened.body else {
            return Err("retained CJ is not an execute".into());
        };
        let claim = document
            .service
            .claims
            .get(key)
            .ok_or("retained CJ claim missing")?;
        let input = nostr::private_artifact::open(&job.input, secret).map_err(|e| e.to_string())?;
        let mut reference = execute.input_artifact.ok_or("retained input pin missing")?;
        let event = reference
            .event
            .take()
            .ok_or("retained declaring event missing")?;
        if *key
            != document
                .service
                .key(&opened.principal, &execute.request, execute.attempt)
            || claim.fingerprint != execute.fingerprint
            || claim.input_digest != execute.input_digest
            || claim.lock_digest != execute.lock.digest
            || claim.record_digest != nostr::contracts::digest_bytes(&job.root)
            || event.id != job.input.id
            || event.pubkey != job.input.pubkey
            || event.kind != 3188
            || input.signer() != opened.principal
            || input.artifact() != &reference
        {
            return Err("retained CJ claim differs from its original request or input".into());
        }
        if let Some(reply) = &job.reply {
            let mut service = document.service.clone();
            let accepted = service
                .acknowledge(key, &job.root)
                .map_err(|e| format!("{e:?}"))?;
            let result = claim
                .result
                .as_ref()
                .ok_or("retained terminal CJ result is missing")?;
            for (event, expected, kind) in [
                (&reply.feedback, &accepted, execution::FEEDBACK_KIND),
                (&reply.result, result, execution::RESULT_KIND),
            ] {
                event.validate_crypto().map_err(|e| e.to_string())?;
                if event.kind != kind
                    || event.pubkey != document.setup.authority
                    || event.tag_values("e").collect::<Vec<_>>() != [job.request.id.as_str()]
                    || event.tag_values("p").collect::<Vec<_>>() != [opened.principal.as_str()]
                {
                    return Err("retained CJ response routing differs".into());
                }
                let peer: secp256k1::XOnlyPublicKey = opened
                    .principal
                    .parse()
                    .map_err(|_| "retained CJ principal")?;
                let plaintext = nostr::nip44::decrypt(
                    &event.content,
                    &nostr::nip44::conversation_key(secret, &peer),
                )
                .map_err(|_| "retained CJ response encryption")?;
                if nostr::contracts::parse_strict(plaintext.as_bytes())
                    .map_err(|e| e.to_string())?
                    != *expected
                {
                    return Err(
                        "retained CJ index differs from its original signed response".into(),
                    );
                }
            }
            for event in &reply.artifacts {
                let opened =
                    nostr::private_artifact::open(event, secret).map_err(|e| e.to_string())?;
                if opened.signer() != document.setup.authority
                    || opened.recipient() != job.request.pubkey
                {
                    return Err("retained response artifact audience differs".into());
                }
            }
        }
    }
    for (id, event) in &document.sources {
        if &event.id != id {
            return Err("retained source index differs".into());
        }
        nostr::private_artifact::open(event, secret).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn signed(event: &Event, body: &Value, authority: &str, secret: &SecretKey) -> Result<()> {
    let opened = nostr::private_artifact::open(event, secret).map_err(|e| e.to_string())?;
    if opened.signer() != authority
        || opened.inline_bytes() != Some(jcs(body).map_err(|e| e.to_string())?.as_slice())
    {
        return Err("retained control index differs from its original signed body".into());
    }
    control::validate(body).map_err(|e| e.to_string())
}
