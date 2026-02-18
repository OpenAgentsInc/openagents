# syntax=docker/dockerfile:1.7

ARG ELIXIR_IMAGE=elixir:1.19.5-otp-28

FROM ${ELIXIR_IMAGE} AS builder

ENV MIX_ENV=prod
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends build-essential git && rm -rf /var/lib/apt/lists/*

RUN mix local.hex --force && mix local.rebar --force

COPY mix.exs mix.lock ./
COPY config config
RUN mix deps.get --only prod
RUN mix deps.compile

COPY lib lib
COPY priv priv
RUN mix compile
RUN mix release

FROM ${ELIXIR_IMAGE} AS runner

ENV MIX_ENV=prod
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates openssl libstdc++6 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/_build/prod/rel/openagents_runtime ./

EXPOSE 4000

CMD ["bin/openagents_runtime", "start"]
