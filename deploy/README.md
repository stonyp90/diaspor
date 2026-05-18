# Deploy

Production deployment artifacts for `diaspor-api-server`.

This directory has three things:

| Path | Purpose |
| --- | --- |
| `docker/diaspor-api-server/Dockerfile` | Multi-stage build (cargo-chef + distroless) producing the runtime image. |
| `helm/diaspor-api-server/` | Helm chart for Kubernetes deployments. The source of truth for what a "production" install looks like. |
| `README.md` | This file. |

The API server binary lives at `crates/diaspor-api-server/`. It binds
`127.0.0.1:7733` by default; for any deployment outside `localhost` set
`DIASPOR_BIND_ADDR=0.0.0.0:7733` (the bundled Dockerfile sets this for
you).

## Option 1 — Local Docker

Fastest way to smoke-test the binary in a container. Build context is
the repo root, not this directory:

```bash
docker build \
    -f deploy/docker/diaspor-api-server/Dockerfile \
    -t diaspor-api-server:dev .

docker run --rm \
    -e DIASPOR_JWT_SECRET=devsecret \
    -p 7733:7733 \
    diaspor-api-server:dev

# In another terminal:
curl http://localhost:7733/v1/health
```

Notes:

- The first build takes a few minutes (cargo-chef cooks the full
  dependency graph). Subsequent builds reuse the deps layer and finish
  in seconds.
- The image runs as `nonroot` (UID 65532) on `gcr.io/distroless/cc-debian12`.
  There is no shell, so `docker exec -it … sh` will not work — use
  `docker logs` instead.

## Option 2 — Plain Kubernetes manifests

There is no separate `manifests/` directory. Use the Helm chart's
[`values.yaml`](helm/diaspor-api-server/values.yaml) as the
source-of-truth for the resource shapes (replicas, security context,
probes, resources, etc.) and translate by hand if you really do not
want Helm in the loop.

If you only need to render the YAML once for a GitOps pipeline:

```bash
helm template diaspor-api ./deploy/helm/diaspor-api-server \
    --set secretEnv.DIASPOR_JWT_SECRET=$(openssl rand -hex 32) \
    > diaspor-api-rendered.yaml
```

Then commit the rendered manifests to your GitOps repo.

## Option 3 — Production Helm install

This is what `api.diaspor.io` itself runs on:

```bash
# Add a JWT signing secret — required, install fails fast without it.
helm install diaspor-api ./deploy/helm/diaspor-api-server \
    --namespace diaspor \
    --create-namespace \
    --set secretEnv.DIASPOR_JWT_SECRET=$(openssl rand -hex 32)

# Verify health:
kubectl -n diaspor port-forward svc/diaspor-api-diaspor-api-server 7733:7733 &
curl http://localhost:7733/v1/health
```

To enable ingress at install time:

```bash
helm install diaspor-api ./deploy/helm/diaspor-api-server \
    --namespace diaspor \
    --create-namespace \
    --set secretEnv.DIASPOR_JWT_SECRET=$(openssl rand -hex 32) \
    --set ingress.enabled=true \
    --set ingress.className=nginx \
    --set ingress.host=api.diaspor.io \
    --set ingress.tlsSecretName=diaspor-api-tls
```

To enable horizontal autoscaling (off by default):

```bash
helm upgrade diaspor-api ./deploy/helm/diaspor-api-server \
    --namespace diaspor \
    --reuse-values \
    --set autoscaling.enabled=true
```

To roll a new JWT secret:

```bash
helm upgrade diaspor-api ./deploy/helm/diaspor-api-server \
    --namespace diaspor \
    --reuse-values \
    --set secretEnv.DIASPOR_JWT_SECRET=$(openssl rand -hex 32)
# The Deployment includes a checksum/secret annotation so pods roll
# automatically on Secret changes.
```

## Verifying the chart

```bash
helm lint deploy/helm/diaspor-api-server/
helm template diaspor-api ./deploy/helm/diaspor-api-server \
    --set secretEnv.DIASPOR_JWT_SECRET=test | kubectl apply --dry-run=client -f -
```

## Versioning

| Component | Source of truth |
| --- | --- |
| Binary version | Workspace `Cargo.toml` (`workspace.package.version`). |
| Image tag | `values.yaml` → `image.tag`. Matches the binary version. |
| Chart version | `Chart.yaml` → `version`. Changes when the chart shape changes. |
| App version | `Chart.yaml` → `appVersion`. Matches the binary version. |

When bumping the API server release, update all four together.
