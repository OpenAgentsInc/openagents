import unittest
from urllib.parse import parse_qs, unquote, urlsplit
from database import database_url


class DatabaseUrl(unittest.TestCase):
    def test_cloud_sql_socket_and_escaped_credentials(self):
        url = database_url(r"host=/cloudsql/example:region:db user=lab dbname='lab data' password='a\'b\\c @:/' sslmode=disable")
        parsed = urlsplit(url)
        self.assertEqual(unquote(parsed.username), 'lab')
        self.assertEqual(unquote(parsed.password), "a'b\\c @:/")
        self.assertEqual(unquote(parsed.path), '/lab data')
        self.assertEqual(parse_qs(parsed.query), {'host': ['/cloudsql/example:region:db'], 'sslmode': ['disable']})

    def test_tcp_and_unquoted_escaping(self):
        url = database_url(r'host=127.0.0.1 port=5433 user=lab dbname=lab password=two\ words connect_timeout=10')
        parsed = urlsplit(url)
        self.assertEqual(unquote(parsed.password), 'two words')
        self.assertEqual(parse_qs(parsed.query)['port'], ['5433'])

    def test_url_is_unchanged(self):
        url = 'postgresql://lab:synthetic@localhost/lab?host=%2Fcloudsql%2Fexample&sslmode=disable'
        self.assertEqual(database_url(url), url)

    def test_unsupported_or_ambiguous_fields_fail(self):
        for dsn in ['host=one,two user=lab dbname=lab', 'host=one user=lab dbname=lab host=two',
                    "host=one user=lab dbname='unclosed", 'host=one user=lab dbname=lab sslrootcert=file']:
            with self.subTest(dsn=dsn), self.assertRaises(ValueError):
                database_url(dsn)


if __name__ == '__main__':
    unittest.main()
