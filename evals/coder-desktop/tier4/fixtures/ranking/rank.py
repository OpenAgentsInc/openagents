# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
def leaderboard(players):
    """Players by score, highest first; equal scores by name A to Z."""
    return sorted(players, key=lambda p: (p['score'], p['name']), reverse=True)
