# Boltz Regtest

An environment for local development and testing, successor of [legend-regtest-enviroment](https://github.com/BoltzExchange/legend-regtest-enviroment).

## Prerequisites
[Docker](https://docs.docker.com/engine/install/) or [Orbstack](https://orbstack.dev/) for Apple Silicon based Macs.
When using OrbStack on macOS, set `export DOCKER_DEFAULT_PLATFORM=linux/amd64` before starting.

## Usage

```bash
./start.sh
```

```bash
./stop.sh
```

- Web App: [http://localhost:8080](http://localhost:8080)
- API: [http://localhost:9001](http://localhost:9001)


Data dirs for the services are stored in `./data` folder.

### Scripts container

```bash
docker exec -it boltz-scripts bash
```

- bitcoin-cli-sim-client
- bitcoin-cli-sim-server
- elements-cli-sim-client
- elements-cli-sim-server
- lightning-cli-sim-client
- lightning-cli-sim-server
- lncli-sim-client
- lncli-sim-server
- boltz-client-cli-sim
- boltz-backend-cli-sim


Or alternatively, you can `source aliases.sh` to have these convenience scripts available on the host machine.

### Block explorers

[Esplora](https://github.com/Blockstream/esplora) is running for the Bitcoin Core and Elements regtest:

- Bitcoin: [http://localhost:4002](http://localhost:4002)
- Elements: [http://localhost:4003](http://localhost:4003)

[Otterscan](https://github.com/otterscan/otterscan) is used as block explorer for Anvil: [http://localhost:5100](http://localhost:5100)

### Your first swap

Run `./start.sh` and enter the scripts container or add aliases on your host as described above.

Check the client's bitcoin wallet balance:

```bash
bitcoin-cli-sim-client getbalance
```

Check the client's LND channel balance:

```bash
lncli-sim-client channelbalance
```

Generate e.g. a Bitcoin -> Lightning swap in [Web App](http://localhost:8080/) using Client's LND as destination with a 100k sats invoice:

```bash
lncli-sim-client addinvoice --amt 100000
```

Send the required Bitcoin amount into the swap, e.g. with

```bash
bitcoin-cli-sim-client sendtoaddress bcrt1qfzjp72mft2kcx49w49l5v6vdegr7lff86j08w7 0.00104200
```

Mine three blocks for the swap to process and watch web app progress to the swap success screen:

```bash
bitcoin-cli-sim-client -generate 3
```

Check the client's bitcoin wallet balance again:

```bash
bitcoin-cli-sim-client getbalance
```

Check the client's LND channel balance again:

```bash
lncli-sim-client channelbalance
```

### Updating images

```bash
./stop.sh
docker compose pull
./start.sh
```
