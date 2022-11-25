# flowsnet
Command line tool for interacting with flows.network platform

## Prerequisite
You need to [install WasmEdge runtime](https://wasmedge.org/book/en/quick_start/install.html) to run the wasm in your local environment.
```
curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- -e all -v 0.11.2
```

## How to install
flowsnet is published as a rust binary crate, so you can install it using cargo:
```
cargo install flowsnet
```

## How to use
flowsnet requires three arguments:
```
-f, --flow <FLOW>  Flow identity in flows.network
-w, --wasm <WASM>  Wasm path in the local file system
-p, --port <PORT>  Port of the local service
```

You can find the flow identity in your flow detail on the flows.network platform.<br/>
The wasm path is the path of the wasm file, which is built from your rust function code.<br/>
flowsnet will start a server to receive requests from the flows.network platform, and the port is for the service to listen.
