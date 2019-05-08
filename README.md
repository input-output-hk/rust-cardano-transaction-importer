# Transaction importer

Index transactions by address using the http bridge and sqlite

## Building

`cargo build --release`

## Initial setup

### Setup and sync to the latest stable epoch

`cargo r --release -- sync-block-index`

*Note: This requires the http-bridge instance to be fully synced*

## Start listening to requests

`cargo r --release -- start`

## Endpoints

### GET /transactions/:address

Get history of transactions of the given address

Parameters:
 - address: The base 58 address

#### Example

##### Request

`
http://localhost:3000/transactions/DdzFFzCqrht3THy8XWeBaDmefLcT7EFtwDuTGfM8pN5aZcuT6Xa48XSgK96KG3RbWTfyYQcBDqXREXhiroHYvKAkqmSXdB2JptgBmRYc
`

##### Response

```JSON
[
    {
        "inputs": [
            {
                "id": "04c0d01d81de2ad9e12e48a2a900663d350d5a3f0c0861677dea6ce5298f78cc",
                "index": 1
            },
            {
                "id": "52fd6de11f8231699fe27f55bbb970ee728ff8887d4667e071cfd02df2cda18f",
                "index": 1
            },
            {
                "id": "cce3394a05fde913017d2c38deaadee5095c19524d5a2fa762bf509484f9adfd",
                "index": 1
            }
        ],
        "outputs": [
            {
                "address": "DdzFFzCqrhsyaEVjnVzxC4VWuVez3BvuNjWayRGe24xCn1Hix9JJskc72VHqVbwEKeQcrAmdbkUTcQz8gSr1Yw8XWD2DuUanw5yE5rhX",
                "value": 6272091600
            },
            {
                "address": "DdzFFzCqrht3THy8XWeBaDmefLcT7EFtwDuTGfM8pN5aZcuT6Xa48XSgK96KG3RbWTfyYQcBDqXREXhiroHYvKAkqmSXdB2JptgBmRYc",
                "value": 100000000
            }
        ],
        "txid": "a62148de78f0054c5f26f7efa1f391eadcc80b871983cd0b8a66bf511b25950a"
    },
    {
        "inputs": [
            {
                "id": "a62148de78f0054c5f26f7efa1f391eadcc80b871983cd0b8a66bf511b25950a",
                "index": 0
            }
        ],
        "outputs": [
            {
                "address": "DdzFFzCqrhse1zGY8AvyaPyLEsYax4tVk2SGAxY9GLzbfEaKgMkiFjhgqbqWB33X4CfG57MuSW58G5FoFzHL52ufLt45ACZRnT9YQKZe",
                "value": 3771920706
            },
            {
                "address": "DdzFFzCqrht3THy8XWeBaDmefLcT7EFtwDuTGfM8pN5aZcuT6Xa48XSgK96KG3RbWTfyYQcBDqXREXhiroHYvKAkqmSXdB2JptgBmRYc",
                "value": 2500000000
            }
        ],
        "txid": "06d4c30520db17418c28d50ecbad6235fc0565a9226c5c451ea417921a5a7b53"
    }
]
```

### GET /transaction/:tx

Get a specific transaction inputs and outputs

Parameters:
 - tx: The hash of the transaction

#### Example

##### Request

`
http://localhost:3000/transaction/a62148de78f0054c5f26f7efa1f391eadcc80b871983cd0b8a66bf511b25950a
`

##### Response

```JSON
{
  "txid": "a62148de78f0054c5f26f7efa1f391eadcc80b871983cd0b8a66bf511b25950a",
  "inputs": [
    {
      "id": "04c0d01d81de2ad9e12e48a2a900663d350d5a3f0c0861677dea6ce5298f78cc",
      "index": 1
    },
    {
      "id": "52fd6de11f8231699fe27f55bbb970ee728ff8887d4667e071cfd02df2cda18f",
      "index": 1
    },
    {
      "id": "cce3394a05fde913017d2c38deaadee5095c19524d5a2fa762bf509484f9adfd",
      "index": 1
    }
  ],
  "outputs": [
    {
      "address": "DdzFFzCqrhsyaEVjnVzxC4VWuVez3BvuNjWayRGe24xCn1Hix9JJskc72VHqVbwEKeQcrAmdbkUTcQz8gSr1Yw8XWD2DuUanw5yE5rhX",
      "value": 6272091600
    },
    {
      "address": "DdzFFzCqrht3THy8XWeBaDmefLcT7EFtwDuTGfM8pN5aZcuT6Xa48XSgK96KG3RbWTfyYQcBDqXREXhiroHYvKAkqmSXdB2JptgBmRYc",
      "value": 100000000
    }
  ]
}
```
### Configuration

The server can be configured with the Settings.toml file

Example

```TOML
http-bridge = "http://localhost:8080/"
port = 3000
network = "mainnet"
refresh-interval = 1000
database = "transactions.db"
```