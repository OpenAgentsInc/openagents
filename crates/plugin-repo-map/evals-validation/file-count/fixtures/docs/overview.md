# Overview

A small HTTP API with two resources, users and orders. Handlers live under
`api/handlers/`, the records they return under `api/models/`, and the route
table in `api/app.py`. There is no database: both stores are in-memory
dictionaries, which is enough for the exercises this project is used in.
