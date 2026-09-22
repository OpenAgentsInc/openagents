"""Generate the acme-returns records file deterministically.

The caller is fictional; the records are authored here, not harvested.
Every record is a returns-desk ticket with a disposition label, written
to `records.jsonl` in the shape `gym build` reads.
"""

import json
from pathlib import Path

QUESTION = {
    "type": "choice",
    "instructions": "Which disposition should the returns desk apply to this record?",
    "criteria": {
        "refund": "Return the customer's money; a billing error or an open-and-return policy applies",
        "replacement": "Ship a new unit; the item failed under warranty terms and the claim is routine",
        "warranty_claim": "Route to the manufacturer's claim process; they owe the remedy",
        "policy_only": "No action; the customer is asking what the policy is",
    },
}

# (family, state, truth) — authored, not sampled from anywhere.
RECORDS = [
    # consumer-returns: routine retail tickets
    ("consumer-returns", {"ticket": "Customer was billed twice for order 4471 and wants the second charge back", "channel": "email"}, "refund"),
    ("consumer-returns", {"ticket": "Blender arrived with a cracked jar; customer wants a new one", "channel": "chat"}, "replacement"),
    ("consumer-returns", {"ticket": "Vacuum motor burned out at 14 months; manufacturer warranty is 2 years", "channel": "phone"}, "warranty_claim"),
    ("consumer-returns", {"ticket": "Does your store accept returns without a receipt?", "channel": "chat"}, "policy_only"),
    ("consumer-returns", {"ticket": "Charged a restocking fee the listing said was waived; refund requested", "channel": "email"}, "refund"),
    ("consumer-returns", {"ticket": "Headphones left channel is dead after a week; wants a swap", "channel": "chat"}, "replacement"),
    ("consumer-returns", {"ticket": "Washing machine bearing failed; covered under parts warranty, not ours", "channel": "phone"}, "warranty_claim"),
    ("consumer-returns", {"ticket": "How long is the return window for sale items?", "channel": "email"}, "policy_only"),
    ("consumer-returns", {"ticket": "Subscription renewed after customer cancelled; wants the charge reversed", "channel": "email"}, "refund"),
    ("consumer-returns", {"ticket": "Laptop battery holds 20 minutes; still inside the 1-year coverage", "channel": "phone"}, "replacement"),
    ("consumer-returns", {"ticket": "Dishwasher control board failed at year three of the maker's five-year plan", "channel": "phone"}, "warranty_claim"),
    ("consumer-returns", {"ticket": "Can I return an opened software package?", "channel": "chat"}, "policy_only"),
    ("consumer-returns", {"ticket": "Duplicate invoice posted after the card was already charged", "channel": "email"}, "refund"),
    ("consumer-returns", {"ticket": "Tablet screen has dead pixels out of the box", "channel": "chat"}, "replacement"),
    ("consumer-returns", {"ticket": "Compressor in the fridge died; the sealed system warranty applies", "channel": "phone"}, "warranty_claim"),
    ("consumer-returns", {"ticket": "Is there a fee for returning large appliances?", "channel": "chat"}, "policy_only"),
    ("consumer-returns", {"ticket": "A discount code was applied then the full price was charged anyway", "channel": "email"}, "refund"),
    ("consumer-returns", {"ticket": "Kettle stopped heating after two weeks of use", "channel": "chat"}, "replacement"),
    ("consumer-returns", {"ticket": "Power supply failed inside the OEM coverage period", "channel": "phone"}, "warranty_claim"),
    ("consumer-returns", {"ticket": "What is the policy on final-sale clearance items?", "channel": "email"}, "policy_only"),
    # pro-returns: commercial accounts
    ("pro-returns", {"ticket": "Fleet account was billed for 12 units and received 10; refund for the two missing", "channel": "portal"}, "refund"),
    ("pro-returns", {"ticket": "Job-site drill failed under load in week one; contractor wants a swap", "channel": "portal"}, "replacement"),
    ("pro-returns", {"ticket": "Generator stator failed; industrial line is covered by the maker directly", "channel": "phone"}, "warranty_claim"),
    ("pro-returns", {"ticket": "Do bulk orders have a different return window?", "channel": "portal"}, "policy_only"),
    ("pro-returns", {"ticket": "Invoice 8804 charged freight the contract caps at zero", "channel": "portal"}, "refund"),
    ("pro-returns", {"ticket": "Thermal camera drifts out of spec after a month; lab needs a working unit", "channel": "portal"}, "replacement"),
    ("pro-returns", {"ticket": "CNC spindle bearing failed inside the OEM's coverage window", "channel": "phone"}, "warranty_claim"),
    ("pro-returns", {"ticket": "Is there a restocking fee on custom-configured gear?", "channel": "portal"}, "policy_only"),
    ("pro-returns", {"ticket": "Duplicate PO processed after the first was already invoiced", "channel": "portal"}, "refund"),
    ("pro-returns", {"ticket": "Pressure gauge reads 8% high fresh from the box", "channel": "portal"}, "replacement"),
    ("pro-returns", {"ticket": "Compressor failed at month ten of the maker's eighteen-month term", "channel": "phone"}, "warranty_claim"),
    ("pro-returns", {"ticket": "What documentation do you need for a warranty return?", "channel": "portal"}, "policy_only"),
    ("pro-returns", {"ticket": "Service contract was renewed at last year's rate but billed at this year's", "channel": "portal"}, "refund"),
    ("pro-returns", {"ticket": "Welder overheats after twenty minutes on the bench", "channel": "portal"}, "replacement"),
    ("pro-returns", {"ticket": "Hydraulic pump failed inside the supplier's parts coverage", "channel": "phone"}, "warranty_claim"),
    ("pro-returns", {"ticket": "Can we return consumables bought in bulk?", "channel": "portal"}, "policy_only"),
    ("pro-returns", {"ticket": "Account was charged for a support tier it never had", "channel": "portal"}, "refund"),
    ("pro-returns", {"ticket": "Label printer skips every third label out of the box", "channel": "portal"}, "replacement"),
    ("pro-returns", {"ticket": "Motor controller failed; the drive maker covers years two and three", "channel": "phone"}, "warranty_claim"),
    ("pro-returns", {"ticket": "What is the cutoff for next-day replacement shipping?", "channel": "portal"}, "policy_only"),
]


def main() -> None:
    out = Path(__file__).parent / "records.jsonl"
    with out.open("w") as file:
        for index, (family, state, truth) in enumerate(RECORDS, 1):
            file.write(
                json.dumps(
                    {
                        "id": f"acme-{index:03d}",
                        "family": family,
                        "kind": "choice",
                        "state": state,
                        "truth": truth,
                        "question": QUESTION,
                        "label_rule": "the returns desk's disposition at close",
                    }
                )
                + "\n"
            )
    print(f"wrote {len(RECORDS)} records to {out}")


if __name__ == "__main__":
    main()
