# flowsnet-cli
Command line tool for interacting with flows.network platform

## How to install
flowsnet-cli is published as a rust binary crate, so you can install it using cargo:
```
cargo install flowsnet-cli
```

## How to use
flowsnet-cli requires three arguments:
```
-f, --flow <FLOW>  Flow identity in flows.network
-w, --wasm <WASM>  Wasm path in the local file system
-p, --port <PORT>  Port of the local service
```

You can find the flow identity in your flow detail on the flows.network platform.<br/>
The wasm path is the path of the wasm file, which is built from your rust function code.<br/>
Flowsnet-cli will start a server to receive requests from the flows.network platform, and the port is for the service to listen.
