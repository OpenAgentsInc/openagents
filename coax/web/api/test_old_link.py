# The old playtest's shared links (pt-cutover-plan): /api/playtest/old-link moves for good to the realm page. test.py runs this file.
import http.client
import urllib.parse


def old_link(query):
    u = urllib.parse.urlsplit(API)
    c = http.client.HTTPConnection(u.hostname, u.port, timeout=5)
    c.request("GET", "/api/playtest/old-link" + query)
    r = c.getresponse()
    r.read()
    return r.status, r.getheader("Location")


R = "https://openagents.com/realm/haunted-house?from=playtest"
check("a shared planet opens that world", old_link("?visit=planet-06258408&planet=forest&name=Kiki") == (301, R + "#planet-06258408"))
check("a link with no world opens the Haunted House", old_link("") == (301, R))
check("the house's own id is the house", old_link("?visit=haunted-house") == (301, R))
check("anything else in visit is dropped", old_link("?visit=%3Cscript%3E") == (301, R) and old_link("?visit=" + "a" * 65) == (301, R))
