import unittest

from observe_vm import tcp_rows


class TcpObservations(unittest.TestCase):
    def test_only_verified_worker_sockets_with_rtt_are_retained(self):
        text = '''ESTAB 0 0 10.1.0.2:45670 10.1.0.3:45601 users:(("worker",pid=42,fd=7))
 cubic wscale:7,7 rtt:0.125/0.031 bytes_sent:261247 bytes_acked:261248 bytes_received:80
ESTAB 0 0 10.1.0.2:22 10.1.0.1:50000 users:(("sshd",pid=99,fd=4))
 cubic rtt:0.500/0.125 bytes_sent:123
ESTAB 0 0 10.1.0.2:45671 10.1.0.4:45602 users:(("worker",pid=42,fd=8))
 cubic bytes_sent:100
CLOSE-WAIT 0 0 10.1.0.2:45672 10.1.0.5:45603 users:(("worker",pid=42,fd=9))
 cubic rtt:0.900/0.100 bytes_sent:999
'''
        rows = tcp_rows(text, {42: {"stage": "s0"}})
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0], {
            "pids": [42], "local": "10.1.0.2:45670", "peer": "10.1.0.3:45601",
            "rtt_ms": 0.125, "rtt_variation_ms": 0.031, "bytes_sent": 261247,
            "bytes_acked": 261248, "bytes_received": 80, "bytes_retrans": None,
        })

    def test_loopback_direction_and_worker_identity_survive_indented_headers(self):
        text = '''  ESTAB 0 0 [::ffff:10.1.0.2]:45670 [::ffff:10.1.0.2]:45601 users:(("worker",pid=42,fd=7))
 bbr rtt:0.010/0.002 bytes_sent:100000 bytes_received:80
ESTAB 0 0 10.1.0.2:45601 10.1.0.2:45670 users:(("worker",pid=43,fd=8))
 bbr rtt:0.011/0.003 bytes_sent:80 bytes_received:100000
'''
        rows = tcp_rows(text, {42: {"stage": "s0"}, 43: {"stage": "s1"}})
        self.assertEqual([r["pids"] for r in rows], [[42], [43]])
        self.assertGreater(rows[0]["bytes_sent"], rows[0]["bytes_received"])
        self.assertLess(rows[1]["bytes_sent"], rows[1]["bytes_received"])
        self.assertIn("::ffff:", rows[0]["peer"])


if __name__ == "__main__":
    unittest.main()
