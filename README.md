# flowsnet-cli
Command line tool for interacting with flows.network platform

## Prerequisite
Your operation system needs to be Linux. And you may need to install clang, pkg-config and openssl.
Take Ubuntu as example:
```
sudo apt install clang pkg-config openssl libssl-dev
```

You need to [install WasmEdge runtime](https://wasmedge.org/book/en/quick_start/install.html) to run the wasm in your local environment.
```
curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- --plugins wasmedge_rustls
```

## How to install
For the reason that flowsnet-cli depends on the version of your WasmEdge, so we recommand you to compile it manullay.

## How to use
flowsnet-cli requires three arguments:
```
  -f, --flow <FLOW>          Flow identity in flows.network
  -w, --wasm <WASM>          Wasm file path in the local file system
  -p, --port <PORT>          Port of the local service
```
and two optionals:
```
  -d, --work-dir <WORK_DIR>  Path for env file and mounting volume in the local file system [default: .]
  -e, --env-file <ENV_FILE>  Name of the env file which is to be written [default: .flowsnet.env]
```

You can find the flow identity in your flow detail on the flows.network platform.<br/>
The wasm path is the path of the wasm file, which is built from your rust function code.<br/>
flowsnet-cli will start a server to receive requests from the flows.network platform, and the port is for the service to listen.
