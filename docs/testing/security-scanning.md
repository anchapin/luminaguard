# Security Scanning Guide

## Overview

LuminaGuard employs comprehensive security scanning as part of our CI/CD pipeline to identify vulnerabilities early in the development process. Security is a core value proposition, and automated scanning helps maintain that standard.

## Security Scan Workflow

The security scanning is integrated into CI through multiple workflows:

1. **`.github/workflows/security-scan.yml`** - Dedicated security scanning workflow
2. **`.github/workflows/ci.yml`** - Integrated security scan in main CI pipeline
3. **Weekly scheduled audits** - Full security audit every Monday at 9:00 UTC

## Security Tools

### 1. Rust Security Audit (cargo-audit)

**Purpose:** Scans Rust dependencies for known vulnerabilities in the RustSec Advisory Database.

**Configuration:**
- Version: 0.20.0
- Scans: `orchestrator/Cargo.lock`
- Fails on: Critical CVEs
- Warns on: High severity CVEs

**Usage in CI:**
```yaml
- name: Run cargo-audit
  working-directory: orchestrator
  run: |
    cargo install cargo-audit --version 0.20.0
    cargo audit --json > audit-report.json
```

**Local Usage:**
```bash
cd orchestrator
cargo install cargo-audit
cargo audit
```

**Severity Levels:**
- **Critical:** Blocks CI/CD - must fix immediately
- **High:** Warning - should fix before next release
- **Medium/Low:** Informational - address at your discretion

### 2. Python Security Scan (Bandit)

**Purpose:** Finds common security issues in Python code.

**Configuration:**
- Scans: `agent/` directory (excludes `.venv`, `tests`, `__pycache__`)
- Skips: B101 (assert_used), B601 (paramiko_calls)
- Focus tests: B201, B301, B401, B501, B601, B701
- Fails on: High severity issues

**Common Issues Detected:**
- **B201:** `flask_debug_true` - Flask debug mode enabled
- **B301:** `pickle` - Use of insecure pickle module
- **B401:** `import_telnetlib` - Telnet is insecure
- **B501:** `request_with_no_cert_validation` - SSL cert verification disabled
- **B601:** `paramiko_calls` - SSH without verification
- **B701:** `jinja2_autoescape_false` - Autoescape disabled

**Bandit Configuration (`.bandit` in `agent/`):**
```toml
[bandit]
exclude_dirs = ['.venv', 'tests', '__pycache__']
skips = ['B101', 'B601']
tests = ['B201', 'B301', 'B401', 'B501', 'B601', 'B701']
```

**Usage in CI:**
```yaml
- name: Run bandit scan
  working-directory: agent
  run: |
    pip install bandit[toml]
    bandit -r . -f json -o bandit-report.json
```

**Local Usage:**
```bash
cd agent
pip install bandit[toml]
bandit -r .
```

**Addressing Issues:**
- High severity issues must be fixed before merging
- Medium severity issues should be addressed or documented
- Low severity issues can be addressed in follow-up work

### 3. Dependency Vulnerability Scan (Safety)

**Purpose:** Scans Python dependencies for known security vulnerabilities.

**Configuration:**
- Scans: `agent/requirements.txt` (generated from `.venv`)
- Database: PyPI vulnerability database
- Fails on: Critical CVEs

**Usage in CI:**
```yaml
- name: Run safety check
  working-directory: agent
  run: |
    pip install safety
    safety check --json > safety-report.json
```

**Local Usage:**
```bash
cd agent
.venv/bin/pip freeze > requirements.txt
pip install safety
safety check
```

**Ignoring Vulnerabilities:**
If you need to ignore a vulnerability temporarily (not recommended for production):
```bash
safety check --ignore <vulnerability-id>
```

### 4. Secret Detection (TruffleHog)

**Purpose:** Detects secrets, API keys, passwords, and other sensitive data in commits.

**Configuration:**
- Scans: Entire repository
- Mode: Only verified secrets (reduces false positives)
- Fails on: Any verified secrets found

**Usage in CI:**
```yaml
- name: Run TruffleHog
  uses: trufflesecurity/trufflehog@4158734f234bd8770128deae2e2975cfab4b66a6
  with:
    path: ./
    base: ${{ github.event.repository.default_branch }}
    head: HEAD
    extra_args: --only-verified
```

**Common Secrets Detected:**
- API keys (AWS, Google Cloud, GitHub, etc.)
- Database connection strings
- Private keys (SSH, TLS)
- Passwords in code
- OAuth tokens

**Prevention:**
- Never commit secrets to the repository
- Use environment variables for sensitive data
- Use `.env` files (added to `.gitignore`)
- Use secret management tools (AWS Secrets Manager, HashiCorp Vault)

## CI Integration

### PR Scans

Security scans run on every pull request:

```yaml
on:
  pull_request:
    branches: [main]
```

**Behavior:**
- Scans run in parallel for faster feedback
- Critical findings block PR merge
- Reports are posted as PR comments
- Artifacts are retained for 30 days

### Branch Protection

The CI workflow requires all security scans to pass:

```
pull_request → security-scan → build-orchestrator → tests → merge
```

If security scans fail:
1. PR cannot be merged
2. Developer receives detailed report
3. Must fix or document exemption

### Weekly Full Audit

A comprehensive security audit runs weekly (Monday 9:00 UTC):

```yaml
schedule:
  - cron: '0 9 * * 1'
```

**Includes:**
- Full Rust audit with detailed report
- Complete Python security scan
- Comprehensive dependency check
- Summary report in GitHub Actions

**Manual Trigger:**
You can manually trigger the weekly audit:
```bash
gh workflow run security-scan.yml
```

Or via GitHub UI:
- Go to Actions tab
- Select "Security Scan" workflow
- Click "Run workflow"

## Report Artifacts

All security scan reports are uploaded as GitHub Actions artifacts:

| Artifact | Content | Retention |
|----------|---------|-----------|
| `rust-audit-report` | `cargo audit` JSON output | 30 days |
| `bandit-security-report` | `bandit` JSON output | 30 days |
| `safety-dependency-report` | `safety` JSON output | 30 days |

**Accessing Reports:**
1. Go to Actions tab
2. Select workflow run
3. Scroll to "Artifacts" section
4. Download desired report

## Local Security Scanning

### Quick Scan

Run all security scans locally:

```bash
# Rust audit
cd orchestrator && cargo audit

# Python security scan
cd agent && pip install bandit[toml] && bandit -r .

# Dependency scan
cd agent && .venv/bin/pip freeze > requirements.txt && pip install safety && safety check
```

### As Make Target

Add security scanning to Makefile (not yet implemented):

```makefile
security-scan:
	@echo "🔒 Running security scans..."
	@cd orchestrator && cargo audit || echo "  ⚠️  Rust vulnerabilities found"
	@cd agent && .venv/bin/pip install bandit[toml] -q && bandit -r . --exclude .venv,tests,__pycache__
	@cd agent && .venv/bin/pip freeze > requirements.txt && pip install safety -q && safety check
	@echo "✅ Security scan complete!"
```

**Usage:**
```bash
make security-scan
```

## Addressing Security Findings

### Critical Vulnerabilities

**Action Required:** Fix immediately

1. Identify vulnerable dependency
2. Check for available patches
3. Update dependency
4. Verify fix with re-scan
5. Document in commit message

**Example:**
```bash
# Update vulnerable crate
cd orchestrator
cargo update serde

# Verify fix
cargo audit
```

### High Severity Issues

**Action Required:** Fix before next release

1. Assess risk level
2. Implement fix or mitigation
3. Update tests
4. Document decision

### Medium/Low Severity Issues

**Action Required:** Document and address

1. Create GitHub issue for tracking
2. Assess priority
3. Schedule fix in backlog
4. Document workaround if needed

## Security Alert Configuration

### GitHub Dependabot

GitHub's Dependabot automatically creates PRs for vulnerable dependencies.

**Configuration (`.github/dependabot.yml`):**
```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/orchestrator"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 10

  - package-ecosystem: "pip"
    directory: "/agent"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 10
```

**Status:** Not yet configured (future enhancement)

### Email Notifications

Configure email alerts for security findings:

```yaml
- name: Send email on failure
  if: failure()
  uses: dawidd6/action-send-mail@v3
  with:
    server_address: smtp.gmail.com
    server_port: 465
    username: ${{ secrets.EMAIL_USERNAME }}
    password: ${{ secrets.EMAIL_PASSWORD }}
    subject: "Security scan failed in ${{ github.repository }}"
    to: security@luminaguard.io
    body: "Security scan failed. See ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}"
```

**Status:** Not yet configured (future enhancement)

### Slack Notifications

Configure Slack alerts for security findings:

```yaml
- name: Notify Slack on critical findings
  if: failure()
  uses: slackapi/slack-github-action@v1
  with:
    payload: |
      {
        "text": "🚨 Security scan failed in ${{ github.repository }}",
        "blocks": [
          {
            "type": "section",
            "text": {
              "type": "mrkdwn",
              "text": "🚨 *Critical security finding* in ${{ github.repository }}\n\n*Workflow:* ${{ github.workflow }}\n*Run:* ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}"
            }
          }
        ]
      }
  env:
    SLACK_WEBHOOK_URL: ${{ secrets.SLACK_WEBHOOK_URL }}
```

**Status:** Not yet configured (future enhancement)

## Best Practices

### Dependency Management

1. **Update regularly:** Keep dependencies current
2. **Review changes:** Check changelogs for security fixes
3. **Pin versions:** Use exact versions in production
4. **Audit imports:** Review new dependencies for security

### Code Security

1. **Never hardcode secrets:** Use environment variables
2. **Validate input:** Sanitize all user inputs
3. **Use secure defaults:** Enable security features by default
4. **Follow OWASP:** Adhere to OWASP guidelines

### Vulnerability Response

1. **Patch quickly:** Apply security patches promptly
2. **Test thoroughly:** Verify fixes don't break functionality
3. **Document changes:** Record what was changed and why
4. **Communicate:** Inform users of security updates

## Troubleshooting

### False Positives

**Cargo-audit:**
```bash
# Ignore specific advisory
cargo audit --ignore <advisory-id>
```

**Bandit:**
Add to `.bandit` config:
```toml
[bandit]
skips = ['B401', 'B601']
```

**Safety:**
```bash
safety check --ignore <vulnerability-id>
```

### Scan Failures

**Issue:** Scan fails but can't find vulnerability
**Solution:**
1. Check the JSON report artifact
2. Verify Cargo.lock is up to date
3. Ensure all dependencies are installed
4. Check for scan tool version mismatch

**Issue:** Scan times out
**Solution:**
1. Reduce scan scope
2. Use caching for dependencies
3. Run scans in parallel
4. Optimize dependency tree

### CI Integration Issues

**Issue:** Security scan doesn't block merge
**Solution:**
1. Verify branch protection settings
2. Check `needs:` dependency in CI workflow
3. Ensure GitHub status checks are required

**Issue:** Reports not posting to PR
**Solution:**
1. Verify GitHub token has `issues: write` permission
2. Check workflow has proper `if:` condition
3. Review step `id:` for output variables

## Metrics and Monitoring

### Key Metrics

Track these security metrics:

1. **Vulnerability Count:** Total vulnerabilities found per scan
2. **Mean Time to Fix (MTTF):** Average time to fix vulnerabilities
3. **False Positive Rate:** Percentage of false positives
4. **Scan Duration:** Time to complete security scans

### Monitoring

**GitHub Actions Dashboard:**
- Track workflow success/failure rates
- Monitor scan duration trends
- Review artifact retention

**Security Dashboard (Future):**
- Aggregate vulnerability reports
- Track remediation progress
- Visualize security posture

## Future Enhancements

### Planned Features

1. **GitHub Advanced Security Integration**
   - Code scanning alerts
   - Secret scanning
   - Dependency graph

2. **Container Scanning**
   - Trivy for container image scanning
   - Dockerfile security analysis

3. **Infrastructure as Code Security**
   - Terraform security scanning (tfsec)
   - Kubernetes policy checks (OPA Gatekeeper)

4. **Dynamic Application Security Testing (DAST)**
   - OWASP ZAP integration
   - API security testing

5. **Security Metrics Dashboard**
   - Real-time vulnerability tracking
   - Compliance reporting
   - Risk scoring

## Resources

### Documentation

- [cargo-audit Documentation](https://github.com/RustSec/cargo-audit)
- [Bandit Documentation](https://bandit.readthedocs.io/)
- [Safety Documentation](https://pyup.io/safety/)
- [TruffleHog Documentation](https://github.com/trufflesecurity/trufflehog)

### Security Standards

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CWE/SANS Top 25](https://cwe.mitre.org/top25/)
- [RustSec Advisory Database](https://rustsec.org/)

### Community

- GitHub Issues: Report security issues via private vulnerability disclosure
- Security Email: security@luminaguard.io (future)
- Security Policy: `SECURITY.md` (future)

## Summary

Security scanning is integrated into LuminaGuard's CI/CD pipeline to catch vulnerabilities early. The workflow includes:

- **Rust audits** via cargo-audit
- **Python scans** via bandit
- **Dependency checks** via safety
- **Secret detection** via TruffleHog

All critical findings block PR merges, and weekly audits provide comprehensive security coverage. Local scanning is supported via Make commands for developer convenience.
