## Setup
Requires `openraft` repo to be located in parent directory (`../openraft`)

```bash
git clone https://github.com/databendlabs/openraft.git
```

## Test Script
```bash
./test-cluster.sh
```

## Run Cluster
```bash
RUST_LOG=info ./run-cluster.sh run
```

## Example Requests
```bash
curl --location 'http://localhost:21001/metrics'

curl --location 'http://localhost:21001/write' \
--header 'Content-Type: application/json' \
--data '{
    "Set": {
        "key": "abc",
        "value": "123"
    }
}'

curl --location 'http://localhost:21001/read' \
--header 'Content-Type: application/json' \
--data '"abc"'

curl --location 'http://localhost:21001/linearizable_read' \
--header 'Content-Type: application/json' \
--data '"abc"'

curl --location 'http://localhost:21001/write' \
--header 'Content-Type: application/json' \
--data '{
    "Del": {
        "key": "abc"
    }
}'
```