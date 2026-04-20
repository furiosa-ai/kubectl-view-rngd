# kubectl-view-rngd

A kubectl plugin that visualizes allocation of the `furiosa.ai/rngd` extended
resource across the nodes of a Kubernetes cluster.

## Usage

```
$ kubectl view-rngd
┌───────┬───────┬─────────────────┐
│ Node  │ Usage │ Pods            │
╞═══════╪═══════╪═════════════════╡
│ node1 │ 8 / 8 │ ns1/pod-foo (4) │
│       │       │ ns1/pod-bar (4) │
├───────┼───────┼─────────────────┤
│ node2 │ 0 / 4 │ -               │
├───────┼───────┼─────────────────┤
│ node3 │ 2 / 8 │ ns1/pod-baz (2) │
├───────┼───────┼─────────────────┤
│ node4 │ 8 / 8 │ ns2/pod-a (4)   │
│       │       │ ns2/pod-b (1)   │
│       │       │ ns2/pod-c (1)   │
│       │       │ ns2/pod-d (1)   │
│       │       │ ns2/pod-e (1)   │
└───────┴───────┴─────────────────┘
```

### Required permissions

The plugin needs read access to:

- `nodes` (cluster-scope, `get`/`list`)
- `pods` (all namespaces, `list`)

### Flags

| Flag              | Default  | Description                                                                  |
| ----------------- | -------- | ---------------------------------------------------------------------------- |
| `--kubeconfig`    | inferred | Path to kubeconfig file. Falls back to `$KUBECONFIG` / `~/.kube/config`.     |
| `--context`       | current  | Kube context to use.                                                         |
| `--include-empty` | off      | Also list nodes without `furiosa.ai/rngd` capacity (shown as `0 / 0` / `-`). |
| `-v`, `--verbose` | off      | Emit debug logging to stderr.                                                |

## Installation

### Recommended: kubectl krew

```bash
# one-time: add the furiosa-ai krew index
kubectl krew index add furiosa https://github.com/furiosa-ai/krew-index

# install
kubectl krew install furiosa/view-rngd

# verify
kubectl view-rngd
```

Upgrade later with:

```bash
kubectl krew update
kubectl krew upgrade view-rngd
```

### Manual install

Download the appropriate release tarball from the [Releases][releases] page,
extract it, and place `kubectl-view-rngd` on your `$PATH`:

```bash
curl -sSfL -o view-rngd.tar.gz \
  "https://github.com/furiosa-ai/kubectl-view-rngd/releases/download/v0.1.0/kubectl-view-rngd_v0.1.0_linux_amd64.tar.gz"
tar -xzf view-rngd.tar.gz
install -m 755 kubectl-view-rngd ~/.local/bin/
kubectl view-rngd
```

### From source

```bash
cargo build --release
install -m 755 target/release/kubectl-view-rngd ~/.local/bin/
```

## Development

```bash
cargo test                     # unit + integration tests
cargo build --release          # local build

just check                     # fmt + clippy
just release VERSION=v0.1.0    # cross-compile + tarball + manifest (needs `cross`)
```

## License

Apache-2.0. See [LICENSE](LICENSE).

[releases]: https://github.com/furiosa-ai/kubectl-view-rngd/releases
