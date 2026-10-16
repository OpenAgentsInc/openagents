-- A chat can be requested again under another logical request key. Preserve
-- every admitted key, including after deletion, rather than allocating twice.
CREATE TABLE computers.create_requests (
    owner text NOT NULL,
    request_key text NOT NULL,
    computer text NOT NULL REFERENCES computers.computers(id),
    PRIMARY KEY (owner, request_key)
);
CREATE INDEX computers_create_requests_computer ON computers.create_requests(computer);
INSERT INTO computers.create_requests (owner, request_key, computer)
SELECT owner, idempotency_key, id FROM computers.computers
WHERE idempotency_key IS NOT NULL;
