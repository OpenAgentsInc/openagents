// Firm reason: 6 (C boundary for pay/wallet.coax and pay/wallet-backup.coax called from Apple Swift host).
#ifndef OA_APPLE_WALLET_CORE_H
#define OA_APPLE_WALLET_CORE_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef uint8_t coax_t_Verdict;
typedef uint8_t coax_t_Pay;
typedef uint8_t coax_t_Event;

typedef struct oa_coax_t_Flow {
  coax_t_Pay state;
  int64_t quote;
  int64_t sends;
} oa_coax_t_Flow;

/* pay/wallet.coax (#authority) */
int64_t metaverse_wallet_sat_to_msat(int64_t sat);
int64_t metaverse_wallet_msat_up(int64_t msat);
int64_t metaverse_wallet_msat_down(int64_t msat);
bool metaverse_wallet_amount_ok(int64_t amount, int64_t low, int64_t high, int64_t spendable);
int64_t metaverse_wallet_digit(int64_t sats, int64_t digit);
int64_t metaverse_wallet_backspace(int64_t sats);
bool metaverse_wallet_fee_high(int64_t amount, int64_t fee);
bool metaverse_wallet_expiring(int64_t now, int64_t expires);
coax_t_Verdict metaverse_wallet_may_send(
    coax_t_Pay state, int64_t amount, int64_t fee, int64_t cap,
    int64_t left, int64_t now, int64_t expires);
oa_coax_t_Flow metaverse_wallet_start(void);
oa_coax_t_Flow metaverse_wallet_step(oa_coax_t_Flow f, coax_t_Event e);
int64_t metaverse_wallet_claim_fee_max(int64_t fastest);

/* pay/wallet-backup.coax (#authority) */
int64_t metaverse_backup_card(int64_t balance, bool logged_in, bool backup, bool counts, bool mine, bool later);
bool metaverse_backup_newer(int64_t seen, int64_t epoch);
int64_t metaverse_limit_local(int64_t amount, int64_t spent, int64_t limit, bool logged_in);
int64_t metaverse_limit_next(int64_t status, bool step_up, bool stepped, bool can, int64_t local);

#ifdef __cplusplus
}
#endif

#endif /* OA_APPLE_WALLET_CORE_H */
