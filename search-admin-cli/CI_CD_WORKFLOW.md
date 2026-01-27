# CI/CD Workflow for search-admin CLI

This document explains how the search-admin CLI is built and deployed using GitHub Actions, and how to use it without local Docker access.

## Overview

```
┌─────────────────────┐
│ Developer           │
│ - Edit code         │
│ - Commit & push     │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ GitHub (main)       │
│ - Merge PR          │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ GitHub Actions      │
│ - Build Docker      │
│ - Push to registry  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Container Registry  │
│ search-admin:latest │
│ search-admin:<sha>  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ kubectl (local/CI)  │
│ - Run commands      │
│ - Uses :latest      │
└─────────────────────┘
```

## Workflow File

**Location:** `.github/workflows/search-admin-deploy.yml`

**Triggers:**
- Push to `main` branch
- Changes to:
  - `search-admin-cli/**`
  - `search-indexer-repository/**`
  - `search-indexer-shared/**`
  - `.github/workflows/search-admin-deploy.yml`

**What it does:**
1. Checks out code
2. Logs into DigitalOcean Container Registry
3. Builds Docker image using `search-admin-cli/Dockerfile`
4. Pushes two tags:
   - `registry.digitalocean.com/geo/search-admin:latest` (always current)
   - `registry.digitalocean.com/geo/search-admin:<git-sha>` (for version tracking)

## How to Use

### First Time Setup

1. **Create and merge PR:**

   ```bash
   git checkout -b feat/search-admin-cli
   git add search-admin-cli/ .github/workflows/search-admin-deploy.yml
   git commit -m "feat: add search-admin CLI for index management"
   git push origin feat/search-admin-cli
   # Create PR and merge to main
   ```

2. **Wait for GitHub Actions:**

   Go to: `https://github.com/your-org/gaia/actions`

   Watch for "Deploy Search Admin CLI" workflow to complete (~5-10 minutes)

3. **Verify image is available:**

   ```bash
   # This will be pulled automatically when you run commands
   # registry.digitalocean.com/geo/search-admin:latest
   ```

4. **Run commands via kubectl:**

   ```bash
   cd search-indexer-deploy/k8s/jobs
   ./search-admin.sh list-indices
   ```

### Daily Usage

Once the image is in the registry, you can run commands without touching Docker:

```bash
cd search-indexer-deploy/k8s/jobs

# All commands use the CI/CD-built image
./search-admin.sh list-indices
./search-admin.sh create-index --version 3
./search-admin.sh reindex --source-version 2 --target-version 3
```

### Making Changes to the CLI

1. **Edit code locally:**

   ```bash
   # Make your changes to search-admin-cli/
   vim search-admin-cli/src/commands/create.rs
   ```

2. **Test locally (optional):**

   ```bash
   cargo build --release --bin search-admin
   export OPENSEARCH_URL="http://localhost:9200"
   ./target/release/search-admin list-indices
   ```

3. **Commit and merge:**

   ```bash
   git add search-admin-cli/
   git commit -m "feat: improve create-index command"
   git push origin main
   # Or create PR and merge
   ```

4. **Wait for CI/CD:**

   GitHub Actions automatically rebuilds and pushes the image

5. **Use the new version:**

   ```bash
   # The :latest tag now points to your new version
   ./search-admin.sh list-indices
   ```

## The search-admin.sh Script

**Location:** `search-indexer-deploy/k8s/jobs/search-admin.sh`

This script is a kubectl wrapper that:
1. Retrieves OpenSearch URL from the `opensearch-credentials` secret
2. Runs a temporary pod using the CI/CD-built image
3. Executes your command
4. Streams output to your terminal
5. Automatically deletes the pod when done

**Under the hood:**
```bash
kubectl run search-admin-<timestamp> \
  --image=registry.digitalocean.com/geo/search-admin:latest \
  --restart=Never \
  --rm \
  -i \
  --env="OPENSEARCH_URL=$OPENSEARCH_URL" \
  -n search \
  -- create-index --version 3
```

## Benefits of This Approach

### ✅ No Local Docker Required

You don't need:
- Docker installed locally
- Registry push permissions
- To remember Docker commands
- To manage local images

### ✅ Consistent Images

- Everyone uses the same CI/CD-built image
- No "works on my machine" issues
- Image is tested in CI/CD pipeline
- Same image in dev, staging, and production

### ✅ Fast Iteration

- No local build time (CI/CD builds once)
- Just run commands immediately
- Changes require merge to main (good for production safety)

### ✅ Version Tracking

Every merge creates a tagged image:
```
registry.digitalocean.com/geo/search-admin:abc1234  (git SHA)
registry.digitalocean.com/geo/search-admin:latest   (current)
```

You can use specific versions if needed:
```bash
IMAGE=registry.digitalocean.com/geo/search-admin:abc1234 \
  ./search-admin.sh list-indices
```

## CI/CD Pipeline Details

### Build Stage

```yaml
- name: Build and push Docker image
  run: |
    docker build -t ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.sha }} \
                 -t ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:latest \
                 -f search-admin-cli/Dockerfile .
    docker push ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.sha }}
    docker push ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:latest
```

**Build context:** Workspace root (`.`)
- Allows access to all workspace members
- Includes dependencies: `search-indexer-repository`, `search-indexer-shared`

**Build time:** ~5-10 minutes
- Rust compilation in Docker
- Multi-stage build (builder + runtime)
- Result: ~100MB image

### Secrets Required

The workflow needs these GitHub secrets:

| Secret | Purpose |
|--------|---------|
| `DIGITALOCEAN_ACCESS_TOKEN` | DigitalOcean API token for registry access |

These should already be configured in your GitHub repository settings.

## Debugging

### Check if image exists

```bash
# Try to pull the image
docker pull registry.digitalocean.com/geo/search-admin:latest

# If this works, the image is in the registry
# If it fails, check the GitHub Actions workflow
```

### Check GitHub Actions logs

1. Go to GitHub repository
2. Click "Actions" tab
3. Find "Deploy Search Admin CLI" workflow
4. Click on the latest run
5. Check logs for errors

### Test command manually

```bash
# Get OpenSearch URL
OPENSEARCH_URL=$(kubectl get secret opensearch-credentials -n search -o jsonpath='{.data.OPENSEARCH_URL}' | base64 -d)

# Run kubectl directly
kubectl run test-search-admin --rm -i --restart=Never \
  --image=registry.digitalocean.com/geo/search-admin:latest \
  --env="OPENSEARCH_URL=$OPENSEARCH_URL" \
  -n search \
  -- list-indices
```

### Use debug logging

```bash
RUST_LOG=debug ./search-admin.sh list-indices
```

## Comparison with Local Docker Workflow

### Traditional Workflow (Not Available to You) ❌

```bash
# Build locally
docker build -f search-admin-cli/Dockerfile -t search-admin:latest .

# Tag for registry
docker tag search-admin:latest registry.digitalocean.com/geo/search-admin:latest

# Push to registry (REQUIRES PERMISSIONS)
docker push registry.digitalocean.com/geo/search-admin:latest  # ❌ Permission denied

# Use the image
kubectl run ... --image=registry.digitalocean.com/geo/search-admin:latest
```

### Our CI/CD Workflow (Available to You) ✅

```bash
# Edit code
vim search-admin-cli/src/commands/create.rs

# Commit and merge to main
git add search-admin-cli/
git commit -m "feat: improve command"
git push origin main

# Wait for CI/CD (automatic)

# Use the image (no Docker needed!)
./search-admin.sh create-index --version 3  # ✅ Works!
```

## When to Use Each Approach

### Use search-admin.sh (kubectl wrapper)

✅ **Best for:**
- Ad-hoc operations
- Testing and development
- Quick index management tasks
- Interactive use

```bash
./search-admin.sh list-indices
./search-admin.sh create-index --version 3
```

### Use Kubernetes Jobs

✅ **Best for:**
- Scheduled operations
- Automated workflows
- CI/CD pipelines
- Long-running operations (reindex)

```bash
kubectl apply -f reindex-job.yaml
kubectl logs -n search job/opensearch-reindex -f
```

### Use manage-index.sh helper

✅ **Best for:**
- Full index migrations
- Multi-step workflows
- When you want guided steps
- Production migrations

```bash
./manage-index.sh full-migration 2 3
```

## FAQ

**Q: Do I need Docker installed?**
A: No! The kubectl wrapper uses the CI/CD-built image.

**Q: How do I update the CLI?**
A: Merge your changes to main. CI/CD rebuilds the image automatically.

**Q: What if I need to test changes before merging?**
A: Build and test locally with `cargo run --bin search-admin`. Only merge when ready.

**Q: Can I use an older version of the CLI?**
A: Yes! Set `IMAGE=registry.digitalocean.com/geo/search-admin:<git-sha>` before running the script.

**Q: How long does CI/CD take?**
A: ~5-10 minutes from merge to available image.

**Q: What if CI/CD fails?**
A: Check GitHub Actions logs. Usually it's a compilation error or test failure.

**Q: Can I run this from CI/CD?**
A: Yes! Use the same kubectl commands in your CI/CD pipeline.

## Summary

This workflow enables you to:

1. ✅ Run index management operations without local Docker
2. ✅ Use consistent, CI/CD-built images
3. ✅ Iterate quickly without building locally
4. ✅ Maintain version control of images
5. ✅ Follow best practices for production systems

The trade-off is that code changes require merging to main to become available, which is actually a good practice for production infrastructure operations!
