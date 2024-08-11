# DynamoDB Deduplication Oracle 

An example of how to perform deduplication using DynamoDB as the persistent state.

Noting and retrieving a key are done with a single query, elminiating many of the race and performance conditions associated with deduplication.
To note a key, use a PUT call including a valid (v4) UUID. The response will include:
 - `cnt`: `number` the number of times the UUID key has been noted by the oracle; `cnt` value of `1` indicates the first time the oracle has seen this key.
 - `lst`: `number` the last time this UUID key was noted by the oracle (unix time, utc).
 - `fst`: `number` the first time this UUID key was noted by the oracle (unix time, utc).

To query a key without noting it, use GET instead, the response is the same as above.

