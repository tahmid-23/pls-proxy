# pls-proxy

Want to use an HTTP proxy instead of a VPN?

pls-proxy will proxy your traffic to your HTTP proxy for some double-proxy awesomeness.

## Example Usage
To traffic proxy to `icanhazip.com` on `127.0.0.1:4444`, run `pls-proxy -P 4444 --by proxy.com:8000 -u username -p password icanhazip.com:80`. The username and password arguments are optional.

For example, you can run `curl --header 'Host: icanhazip.com' 127.0.0.1:4444` to receive the proxied output.

## Building
Run `cargo build --release`. This will output to `target/release/pls-proxy`.
