#!/usr/bin/env python3
"""Builds `support-v2.json`, the labelled suite Lev is scored and fitted on.

The items are authored here rather than drawn from an external dataset, so
the labels are the author's and the suite carries a content digest. Size is
the point: `support-v1` held 52 items, which was too few to fit a calibration
map on — every map it produced was refused for degrading the raw signal or
for sitting below the evidence floor. A five-bin table wants tens of items
per bin.

Difficulty is mixed deliberately. A suite of easy items produces a
near-degenerate distribution with no range to calibrate, which is exactly
what the first behavior record found. Roughly a third of these are written to
sit near a boundary, and some are genuinely arguable; the label is the best
reading, not the only one.

    python3 build_support_v2.py > support-v2.json
"""

import hashlib
import json
import sys

ROUTE_OPTS = {
    "billing": "Charges, invoices, refunds, and payment problems",
    "technical": "Bugs, crashes, outages, and performance",
    "sales": "Quotes, plans, upgrades, renewals, and pricing",
}
SEV = [
    "Cosmetic; the product still works",
    "Impaired; there is a workaround",
    "Blocking; there is no workaround",
]

# (state, truth)
ROUTING = [
    # --- billing, clear ---
    ("I was charged twice for the same order and want one refunded.", "billing"),
    ("My invoice shows tax for a state we do not operate in.", "billing"),
    ("The card on file expired and the payment bounced.", "billing"),
    ("Can you refund the prorated amount from last month?", "billing"),
    ("I see a charge from you that I do not recognize at all.", "billing"),
    ("Please send a copy of the receipt for the March payment.", "billing"),
    ("We were billed in dollars but our contract says euros.", "billing"),
    ("The annual charge hit a month earlier than I expected.", "billing"),
    ("Our purchase order number is missing from the invoice.", "billing"),
    ("I cancelled in June and was charged again in July.", "billing"),
    ("The refund you issued never arrived on my statement.", "billing"),
    ("Can you switch us from monthly to annual billing?", "billing"),
    ("We need invoices sent to accounts payable, not to me.", "billing"),
    ("The discount code was applied to the wrong line item.", "billing"),
    ("Why does this month's invoice include a setup fee again?", "billing"),
    ("Our bank flagged your charge as international and blocked it.", "billing"),
    ("I need a VAT receipt for the last three payments.", "billing"),
    ("The invoice total does not match the sum of its lines.", "billing"),
    ("We paid by wire two weeks ago and it still shows unpaid.", "billing"),
    ("Please remove the saved card from our account.", "billing"),
    # --- technical, clear ---
    ("The app crashes every time I open the settings screen.", "technical"),
    ("Exports have been timing out since yesterday afternoon.", "technical"),
    ("Two-factor codes are arriving ten minutes late.", "technical"),
    ("The dashboard shows stale numbers until I hard refresh.", "technical"),
    ("Our webhook endpoint stopped receiving events on Tuesday.", "technical"),
    ("Search returns nothing for terms that clearly exist.", "technical"),
    ("Uploads over about ten megabytes fail with a generic error.", "technical"),
    ("The mobile app shows a blank screen after the splash.", "technical"),
    ("Sorting a table by date puts December before January.", "technical"),
    ("The API returns 502 roughly one request in twenty.", "technical"),
    ("Session cookies expire after a minute instead of a day.", "technical"),
    ("Attachments download with the wrong file extension.", "technical"),
    ("The editor loses my cursor position after every save.", "technical"),
    ("Notifications fire twice for the same event.", "technical"),
    ("Dark mode leaves several labels unreadable.", "technical"),
    ("The integration with our calendar duplicates every meeting.", "technical"),
    ("Pasting from Word inserts invisible characters.", "technical"),
    ("The report builder hangs when I add a fourth filter.", "technical"),
    ("Our SSO login loops back to the sign-in page.", "technical"),
    ("Timestamps display in UTC despite the timezone setting.", "technical"),
    # --- sales, clear ---
    ("Can you send me a quote for 50 seats on the enterprise plan?", "sales"),
    ("What is the difference between the team and business tiers?", "sales"),
    ("We want to add ten seats before the quarter closes.", "sales"),
    ("Do you offer nonprofit pricing?", "sales"),
    ("Our renewal is coming up and I want to compare tiers.", "sales"),
    ("Is there a discount for paying annually instead of monthly?", "sales"),
    ("Can we pilot the enterprise features for a month?", "sales"),
    ("Who would we talk to about a multi-year agreement?", "sales"),
    ("Does the higher tier include the audit log?", "sales"),
    ("We are evaluating you against a competitor and need a comparison.", "sales"),
    ("Can you hold our current price through next year?", "sales"),
    ("What does support look like on the business plan?", "sales"),
    ("We need a security review before we can purchase.", "sales"),
    ("Is there a volume break above a hundred seats?", "sales"),
    ("Can we pay by invoice instead of card if we go annual?", "sales"),
    ("Do you have a reseller programme?", "sales"),
    ("We want to downgrade at renewal rather than cancel.", "sales"),
    ("Does the enterprise tier come with a dedicated contact?", "sales"),
    ("Can you quote us with and without the analytics add-on?", "sales"),
    ("What happens to our data limits if we move up a tier?", "sales"),
    # --- near-boundary and genuinely arguable ---
    ("My card was declined when the plan tried to upgrade itself.", "billing"),
    ("We added seats last month and the invoice does not match the quote.", "billing"),
    ("Everything feels slower since the update, but it might be my laptop.", "technical"),
    ("The upgrade button does nothing when I click it.", "technical"),
    ("I want to cancel because the sync keeps failing.", "technical"),
    ("You charged me for a feature that has been broken all month.", "billing"),
    ("We are over our seat limit and cannot add anyone.", "sales"),
    ("The trial ended early and now we are locked out.", "billing"),
    ("Our admin left and nobody can access the billing portal.", "billing"),
    ("The plan comparison page will not load, so I cannot choose.", "technical"),
    ("We want more storage; is that a plan change or a setting?", "sales"),
    ("The usage meter says we are at 120% but the dashboard says 60%.", "technical"),
    ("I was promised a discount on a call and it is not on the invoice.", "billing"),
    ("Can you tell me why our bill doubled this month?", "billing"),
    ("The enterprise trial has a bug that blocks our evaluation.", "technical"),
    ("We need to move three seats from one workspace to another.", "sales"),
    ("An old employee is still being billed as an active seat.", "billing"),
    ("The API rate limit on our plan is too low for our load.", "sales"),
    ("Exports are slow and we think a higher tier would fix it.", "sales"),
    ("Our invoice lists a seat count we never agreed to.", "billing"),
    ("The renewal quote arrived but the link to accept it is broken.", "technical"),
    ("Do we lose our history if we downgrade?", "sales"),
    ("The payment page throws an error when I enter a new card.", "technical"),
    ("We were told the migration was included and now it is billed.", "billing"),
    ("Our seats show as unused but everyone is logged in.", "technical"),
    ("I would like a refund because the outage cost us a day.", "billing"),
    ("Can we get the audit log without moving to enterprise?", "sales"),
    ("The invoice pdf is corrupted and will not open.", "technical"),
    ("We are being charged for a workspace we deleted.", "billing"),
    ("Single sign-on is listed on our plan but not enabled.", "technical"),
    ("Our contract renews automatically and we want to stop that.", "sales"),
    ("Two of our users cannot be invited; the limit seems wrong.", "technical"),
    ("Is the data residency option a paid add-on?", "sales"),
    ("The credit from last month was never applied.", "billing"),
    ("We want to consolidate two accounts into one subscription.", "sales"),
    ("Password reset emails never arrive for our domain.", "technical"),
    ("Can you explain the overage line on this invoice?", "billing"),
    ("The feature we bought for is not in our plan after all.", "sales"),
    ("Our webhook secret rotated on its own and broke the integration.", "technical"),
    ("We need a W-9 before we can pay this invoice.", "billing"),
]

# (state, urgent)
URGENCY = [
    ("The production site has been down for twenty minutes.", True),
    ("Customer data appears to be visible to the wrong account.", True),
    ("All payments are failing at checkout right now.", True),
    ("We cannot log in and a board demo starts in an hour.", True),
    ("Our webhook has been silently dropping events since Friday.", True),
    ("A user reports seeing another user's invoice.", True),
    ("One of our five integrations disconnected overnight.", True),
    ("We are seeing intermittent 500s on about a tenth of requests.", True),
    ("The API has been returning 401 for every key since the deploy.", True),
    ("Our nightly export has failed three nights running.", True),
    ("Someone deleted the production workspace by accident.", True),
    ("Two-factor is rejecting valid codes for the whole team.", True),
    ("Search has been returning empty results company-wide since noon.", True),
    ("The audit log stopped recording changes yesterday.", True),
    ("A customer's card was charged five times in one minute.", True),
    ("Our admin account appears to have been accessed from abroad.", True),
    ("Uploads are corrupting files rather than failing outright.", True),
    ("The status page says operational but nothing is working.", True),
    ("Sync has been silently dropping records for an unknown period.", True),
    ("We are locked out of the billing portal and renewal is today.", True),
    ("The export finished but the column order changed.", False),
    ("Could you add a dark mode at some point?", False),
    ("The help article for billing has a broken link.", False),
    ("I would like to rename our workspace.", False),
    ("The tooltip on the settings page has a typo.", False),
    ("When is the next release planned?", False),
    ("Sync is slow but it does eventually complete.", False),
    ("The mobile app logs me out every few days.", False),
    ("Can you add CSV as an export format?", False),
    ("The onboarding checklist still shows a step we finished.", False),
    ("Our logo looks slightly stretched in the header.", False),
    ("Is there a keyboard shortcut for the search box?", False),
    ("The docs describe an option that seems to have moved.", False),
    ("Could the date picker default to this month?", False),
    ("We would like an extra admin seat next quarter.", False),
    ("The email footer still lists last year's address.", False),
    ("A column header is truncated on narrow screens.", False),
    ("Can we get a monthly usage summary by email?", False),
    ("The empty state illustration does not match our theme.", False),
    ("I noticed a spelling mistake in the welcome email.", False),
    # boundary cases, where reasonable people differ
    ("One user cannot log in; everyone else is fine.", False),
    ("Reports are running about three times slower than usual.", False),
    ("A scheduled job ran twice and sent duplicate emails to customers.", True),
    ("Our test environment is down; production is unaffected.", False),
    ("The billing page shows an error but charges are going through.", False),
    ("A third of our webhooks are arriving late by several hours.", True),
    ("We cannot add new users, but existing ones work.", False),
    ("An integration token expires tomorrow and we cannot rotate it.", True),
    ("Customers see a stale price on the public pricing page.", True),
    ("The mobile app crashes on one older phone model.", False),
    ("Exports contain a column of nulls that used to hold data.", True),
    ("Our staging database was restored over with production data.", True),
    ("The search index is a day behind.", False),
    ("Permissions changes are taking an hour to take effect.", False),
    ("A customer says they were billed after cancelling.", True),
    ("Attachments over a certain size silently fail to save.", True),
    ("The dashboard loads slowly during our morning peak.", False),
    ("We received a security questionnaire due end of week.", False),
    ("Our SSL certificate expires in three days.", True),
    ("A report shows numbers that contradict the dashboard.", True),
]

# (state, level)
SEVERITY = [
    ("A label is misaligned by two pixels on the settings page.", 0),
    ("The footer year still says last year.", 0),
    ("A tooltip renders below the button instead of above it.", 0),
    ("The loading spinner keeps spinning after content appears.", 0),
    ("Our logo is slightly blurry on high-resolution screens.", 0),
    ("The empty state text has a double space.", 0),
    ("A button's hover colour does not match the design.", 0),
    ("The page title is capitalised inconsistently.", 0),
    ("An icon is a shade off from the rest of the toolbar.", 0),
    ("The success message disappears a little too quickly.", 0),
    ("Table rows lose their zebra striping when filtered.", 0),
    ("The favicon is the old one.", 0),
    ("Exports fail in Safari but work in Chrome.", 1),
    ("Bulk edit fails over 500 rows; smaller batches work.", 1),
    ("The search filter resets when you page back, so you re-enter it.", 1),
    ("Notifications arrive late but they do arrive.", 1),
    ("The mobile app needs a restart once a day to sync.", 1),
    ("Sorting by one column is wrong; the others are fine.", 1),
    ("Keyboard navigation skips the second menu, which is reachable by mouse.", 1),
    ("Attachments must be renamed before they upload successfully.", 1),
    ("The API works but the SDK needs a manual retry.", 1),
    ("Two-factor works by app but not by SMS.", 1),
    ("Reports render but take four minutes instead of ten seconds.", 1),
    ("One integration must be reconnected each week.", 1),
    ("Nobody on the team can log in at all.", 2),
    ("Saving a document silently discards the changes.", 2),
    ("The API returns 500 for every request.", 2),
    ("Payments cannot be completed by any customer.", 2),
    ("The database is refusing writes.", 2),
    ("All exports produce empty files.", 2),
    ("The service is unreachable from every region.", 2),
    ("Data from one customer is appearing in another's account.", 2),
    ("Every scheduled job has stopped running.", 2),
    ("The mobile app crashes on launch for all users.", 2),
    ("Authentication accepts any password.", 2),
    ("The most recent deploy deleted user records.", 2),
]


def build():
    items = []
    for index, (state, truth) in enumerate(ROUTING):
        items.append(
            {
                "id": f"routing/{index:03d}",
                "family": "routing",
                "kind": "choice",
                "state": state,
                "question": {
                    "type": "choice",
                    "instructions": "Which team should handle this message?",
                    "criteria": ROUTE_OPTS,
                },
                "truth": truth,
            }
        )
    for index, (state, urgent) in enumerate(URGENCY):
        items.append(
            {
                "id": f"urgency/{index:03d}",
                "family": "urgency",
                "kind": "noul",
                "state": state,
                "question": {
                    "type": "noul",
                    "instructions": "Does this need urgent attention today?",
                    "criteria": {
                        "true": "Something is broken, blocked, or unsafe right now",
                        "false": "It can wait for the normal queue",
                    },
                },
                "truth": "yes" if urgent else "no",
            }
        )
    for index, (state, level) in enumerate(SEVERITY):
        items.append(
            {
                "id": f"severity/{index:03d}",
                "family": "severity",
                "kind": "score",
                "state": state,
                "question": {
                    "type": "score",
                    "instructions": "How severe is the reported issue?",
                    "criteria": SEV,
                },
                "truth": str(level),
            }
        )

    # Alternate within each family so both splits see the same difficulty mix.
    per_family = {}
    for item in items:
        seen = per_family.setdefault(item["family"], 0)
        item["split"] = "calibration" if seen % 2 == 0 else "evaluation"
        per_family[item["family"]] = seen + 1

    suite = {
        "name": "support-v2",
        "created": "2026-09-19",
        "description": (
            "Support-desk judgments authored in this repository. Three families with "
            "difficulty mixed on purpose: clear cases, near-boundary cases, and some "
            "that are genuinely arguable. Labels are the author's best reading, not "
            "the only defensible one, and they are not drawn from an external dataset. "
            "Sized so a calibration map has enough evidence per bin to be worth fitting."
        ),
        "items": items,
    }
    blob = json.dumps(items, sort_keys=True, separators=(",", ":")).encode()
    suite["digest"] = hashlib.sha256(blob).hexdigest()
    return suite


if __name__ == "__main__":
    json.dump(build(), sys.stdout, indent=1)
    sys.stdout.write("\n")
