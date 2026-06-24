# QA flow — run & post a video to a PR

`run-and-post.sh <PR>` runs a Khala-driven QA session, composes a polished video
(`apps/qa-runner compose`, ffmpeg), and posts it to the PR with **gh-attach**
(`projects/repos/gh-attach`) — the GitHub web-upload path the REST API lacks.

This is the manual/owner version of the automated CI loop (#6185). It proves the
end-to-end flow Rhys wants: agent drives real tools → video → distilled committed
test → evidence posted to the PR. Khala runs on own-infra at $0 via the
operator-credit exemption (#6180).
