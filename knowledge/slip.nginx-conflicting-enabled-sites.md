---
id: slip.nginx-conflicting-enabled-sites
version: 1
kind: slip
title: Resolve conflicting enabled Nginx virtual hosts before adding one
summary: >-
  A syntactically valid Nginx configuration can still contain duplicate
  listen/server-name pairs, causing a warning and one server block to be
  ignored; inspect and resolve existing enabled sites before deploying a new
  one.
tags: [nginx, configuration, deployment]
applies_when: >-
  Adding an Nginx server block on a port/name that may already be configured
  by a default or pre-existing site.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - git-multibranch
  cites:
    - NGINX, Beginner’s Guide, Setting Up a Simple Proxy Server
    - NGINX, Command-line parameters, -t
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details
Before enabling a virtual host, search all included configuration files for the intended listen address/port and `server_name`, then inspect the enabled configuration rather than only the file being edited. Nginx may accept the configuration while warning that a conflicting server name is ignored; a successful syntax test therefore does not establish that requests reach the intended document root or certificate. Disable or reconcile the conflicting site, test configuration, reload, and make an HTTPS request to validate routing and content.

Source: NGINX, *Beginner’s Guide*, “Setting Up a Simple Proxy Server” (server blocks and request selection); NGINX, *Command-line parameters*, `-t` configuration test.
