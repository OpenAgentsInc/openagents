# Mezo Earn Automation

Use this reference when the task is to automate Mezo Earn operations through an agent (lock management, voting, poking, rewards claims, and gauge incentives).

## Source Of Truth Used

- User docs:
  - `~/code/mezo/documentation/src/content/docs/docs/users/mezo-earn/overview/index.mdx`
  - `~/code/mezo/documentation/src/content/docs/docs/users/mezo-earn/lock/vebtc/*.mdx`
  - `~/code/mezo/documentation/src/content/docs/docs/users/mezo-earn/vote/*.mdx`
- Developer docs:
  - `~/code/mezo/documentation/src/content/docs/docs/developers/features/mezo-earn/*.md`
- Contract surface and deployed addresses:
  - `~/code/mezo/tigris/solidity/README.md`
  - `~/code/mezo/tigris/solidity/deployments/{mainnet,testnet}/*.json`
  - `~/code/mezo/tigris/solidity/contracts/interfaces/IVotingEscrow.sol`
  - `~/code/mezo/tigris/solidity/contracts/interfaces/IVoter.sol`
  - `~/code/mezo/tigris/solidity/contracts/interfaces/IRewardsDistributor.sol`
  - `~/code/mezo/tigris/solidity/contracts/interfaces/IReward.sol`

## Epoch And Reward Constraints

- Epoch length is 7 days.
- Epoch start is Thursday 00:00 UTC.
- Votes do not persist across epochs; vote each epoch if fees/incentives are expected.
- Boost changes require `poke` before they affect effective weight.
- Rebase/chain-fee claims are available up to 50 epochs.

## Contract Map (From Tigris Deployments)

## Mainnet (`31612`)
- `VeBTC`: `0x7D807e9CE1ef73048FEe9A4214e75e894ea25914`
- `VeBTCVoter`: `0x3A4a6919F70e5b0aA32401747C471eCfe2322C1b`
- `VeBTCRewardsDistributor`: `0x535E01F948458E0b64F9dB2A01Da6F32E240140f`
- `PoolFactory` (basic pools): `0x83FE469C636C4081b87bA5b3Ae9991c6Ed104248`
- CL pool factory (from Mezo docs): `0xBB24AF5c6fB88F1d191FA76055e30BF881BeEb79`

## Testnet (`31611`)
- `VeBTC`: `0xB63fcCd03521Cf21907627bd7fA465C129479231`
- `VeBTCVoter`: `0x72F8dd7F44fFa19E45955aa20A5486E8EB255738`
- `VeBTCRewardsDistributor`: `0x10B0E7b3411F4A38ca2F6BB697aA28D607924729`
- `PoolFactory` (basic pools): `0x4947243CC818b627A5D06d14C4eCe7398A23Ce1A`

## Core Method Surface For Automation

## VeBTC (`IVotingEscrow`)
- `createLock(uint256 _value, uint256 _lockDuration)`
- `depositFor(uint256 _tokenId, uint256 _value)`
- `increaseAmount(uint256 _tokenId, uint256 _value)`
- `increaseUnlockTime(uint256 _tokenId, uint256 _lockDuration)`
- `withdraw(uint256 _tokenId)`
- `locked(uint256 _tokenId)`
- `balanceOfNFT(uint256 _tokenId)`

## VeBTCVoter (`IVoter`)
- `vote(uint256 _tokenId, address[] _poolVote, uint256[] _weights)`
- `reset(uint256 _tokenId)`
- `poke(uint256 _tokenId)`
- `claimFees(address[] _fees, address[][] _tokens, uint256 _tokenId)`
- `claimBribes(address[] _bribes, address[][] _tokens, uint256 _tokenId)`
- `claimRewards(address[] _gauges)` (for LP stakers)
- `createGauge(address _poolFactory, address _pool)`
- `gauges(address _pool)`, `gaugeToFees(address _gauge)`, `gaugeToBribe(address _gauge)`
- `epochVoteStart(uint256 _timestamp)`, `epochVoteEnd(uint256 _timestamp)`, `epochStart(uint256 _timestamp)`, `epochNext(uint256 _timestamp)`

## VeBTCRewardsDistributor (`IRewardsDistributor`)
- `claimable(uint256 tokenId)`
- `claim(uint256 tokenId)`
- `claimMany(uint256[] tokenIds)`

## Reward Contracts (`IReward`)
- `notifyRewardAmount(address token, uint256 amount)` for incentive top-ups (requires token approval first)
- `getReward(uint256 tokenId, address[] tokens)` for direct reward-contract claims

## Agent Automation Loop (Recommended)

1. Preflight:
- Verify RPC chain id and signer account.
- Resolve contract addresses from known map or deployment JSON.

2. Epoch window check:
- Query vote window using `epochVoteStart(now)` and `epochVoteEnd(now)`.
- If outside vote window, schedule next vote at `epochVoteStart(epochNext(now))`.

3. Lock maintenance:
- Query `locked(tokenId)` and expiration.
- If near-expiry, call `increaseUnlockTime`.
- If strategy calls for increasing stake, call `increaseAmount` (or `depositFor`).

4. Voting:
- Submit `vote(tokenId, pools[], weights[])` during active vote window.
- Call `poke(tokenId)` after vote and after boost-related changes.

5. Claims:
- Call `VeBTCRewardsDistributor.claim(tokenId)` (or `claimMany`) when `claimable(tokenId)` is above threshold.
- Resolve fees/bribes contracts via `gauges(pool)`, `gaugeToFees(gauge)`, and `gaugeToBribe(gauge)`.
- Claim via `claimFees` and `claimBribes`.

6. Incentives:
- To attract votes, approve incentive token to bribe contract and call `notifyRewardAmount(token, amount)`.
- Incentive tokens must be whitelisted in voter.

7. Safety:
- Dry-run calls first (`cast call`) and enforce max slippage / per-tx spend caps.
- Persist last successful epoch actions to avoid duplicate submissions.

## Minimal CLI Patterns (`cast`)

```bash
# Required env
export RPC_URL="https://rpc-http.mezo.boar.network"
export CHAIN_ID="31612"
export PRIVATE_KEY="0x..."
export VEBTC="0x7D807e9CE1ef73048FEe9A4214e75e894ea25914"
export VOTER="0x3A4a6919F70e5b0aA32401747C471eCfe2322C1b"
export DISTRIBUTOR="0x535E01F948458E0b64F9dB2A01Da6F32E240140f"
```

```bash
# Epoch windows
NOW="$(date +%s)"
cast call "$VOTER" "epochVoteStart(uint256)(uint256)" "$NOW" --rpc-url "$RPC_URL"
cast call "$VOTER" "epochVoteEnd(uint256)(uint256)" "$NOW" --rpc-url "$RPC_URL"
cast call "$VOTER" "epochNext(uint256)(uint256)" "$NOW" --rpc-url "$RPC_URL"
```

```bash
# Lock BTC into veBTC
# 1) approve BTC token to VeBTC (set BTC_TOKEN from contracts reference)
cast send "$BTC_TOKEN" "approve(address,uint256)" "$VEBTC" "$LOCK_AMOUNT_WEI" --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY"
# 2) create lock (duration in seconds, rounded to weeks by contract)
cast send "$VEBTC" "createLock(uint256,uint256)" "$LOCK_AMOUNT_WEI" "$LOCK_DURATION_SECONDS" --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY"
```

```bash
# Vote and poke
cast send "$VOTER" "vote(uint256,address[],uint256[])" "$TOKEN_ID" "[$POOL_A,$POOL_B]" "[7000,3000]" --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY"
cast send "$VOTER" "poke(uint256)" "$TOKEN_ID" --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY"
```

```bash
# Claim distributor rewards
cast call "$DISTRIBUTOR" "claimable(uint256)(uint256)" "$TOKEN_ID" --rpc-url "$RPC_URL"
cast send "$DISTRIBUTOR" "claim(uint256)" "$TOKEN_ID" --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY"
```

```bash
# Resolve reward contracts for a pool
GAUGE="$(cast call "$VOTER" "gauges(address)(address)" "$POOL_A" --rpc-url "$RPC_URL" | tr -d '\n')"
FEE_REWARD="$(cast call "$VOTER" "gaugeToFees(address)(address)" "$GAUGE" --rpc-url "$RPC_URL" | tr -d '\n')"
BRIBE_REWARD="$(cast call "$VOTER" "gaugeToBribe(address)(address)" "$GAUGE" --rpc-url "$RPC_URL" | tr -d '\n')"
```

```bash
# Post incentive on bribe reward contract (token must be whitelisted)
cast send "$INCENTIVE_TOKEN" "approve(address,uint256)" "$BRIBE_REWARD" "$INCENTIVE_AMOUNT" --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY"
cast send "$BRIBE_REWARD" "notifyRewardAmount(address,uint256)" "$INCENTIVE_TOKEN" "$INCENTIVE_AMOUNT" --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY"
```

## Agent Task Template

For recurring automation, keep a local strategy file with:
- `token_id`
- target pools and weights
- reward claim threshold
- lock-extension threshold
- incentive budget caps

Then run an epoch scheduler:
- T-10 minutes before `epochVoteEnd`: refresh and submit final vote allocation.
- Immediately after vote submission: `poke`.
- Shortly after epoch flip: claim distributor + fees/bribes if above threshold.
