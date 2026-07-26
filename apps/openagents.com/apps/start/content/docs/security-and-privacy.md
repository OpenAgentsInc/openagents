---
title: Security and privacy
description: The authority, data, credential, and service boundaries for Omega.
lastModified: 2026-07-25
sidebar:
  order: 6
---

## Native authority

Omega is a native application. The host process owns project and local execution access. UI projections must receive bounded data and typed actions, not unrestricted credentials or host authority.

An agent action uses the authority of its selected environment. Omega does not grant repository, deployment, payment, or release authority by displaying that action.

## Application identity and data

Omega must use its own application identity, data directories, logs, cache, credentials, protocol handlers, and update state. It must not read or change Zed user data as part of normal Omega operation.

Prerelease evaluation should use the candidate-specific Omega identity. Do not point an Omega candidate at a Zed data directory.

## Credentials

Provider and account credentials stay with their owning system. External agents keep their own configuration and credential custody.

Do not put credentials, private prompts, repository secrets, private transcripts, or unrestricted local paths in logs, screenshots, documentation, or public evidence.

## Inherited services

Omega is a fork of Zed, but it is not authorized to present Zed services as Omega services. A release candidate must disable inherited production endpoints by default or identify an approved compatibility use.

An unavailable Omega service must have an honest state. It must not silently call an inherited account, telemetry, crash, update, or hosted-model endpoint.

## Release boundary

Signed and notarized bytes are necessary release evidence. They do not prove that privacy, network, data isolation, accessibility, or installed-journey gates passed.

The current public Omega evidence still has open release gates. Do not infer stable support from a prerelease artifact.
