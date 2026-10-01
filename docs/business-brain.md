# Governed Business Brains

Cerebrumma can turn a project Brain into a versioned source of truth for AI
agents. The source remains human-readable Markdown; only reviewed entries are
allowed into the runtime snapshot.

## Safety model

Business memories are opt-in. Existing episodic and semantic memories remain
private development context unless their frontmatter explicitly contains
`runtime: true`.

Every approved runtime entry must declare:

- a stable `id`;
- `status: approved`;
- an `authority` such as `product_truth`, `company_policy`, or `brand_voice`;
- the human or team `owner`;
- one or more agent roles in `scope`;
- whether it is `customer_safe`;
- an `autonomy_level`: `observe`, `draft`, `act`, or `autopilot`.

Optional `effective_at`, `expires_at`, `conflict_key`, and `supersedes` fields
make changes explicit. Two active entries cannot claim the same
`conflict_key` and scope unless one supersedes the other.

Dream-generated knowledge cannot be approved without a `reviewed_by` value.
Reflection may propose business truth, but it cannot silently become business
truth.

Start from [`templates/business-entry.md`](../templates/business-entry.md).

## Validate before agents act

```sh
cerebrum validate
cerebrum validate --json
```

Validation rejects malformed metadata, duplicate IDs, invalid autonomy levels,
unknown supersession targets, conflicting active truth, and unreviewed dream
output. Expired approvals are reported and excluded from publication.

## Publish a runtime snapshot

```sh
cerebrum publish
cerebrum publish --output ./dist/company-brain.json
```

The default output is `.cerebrum/runtime/brain.json`. Publication includes only
approved, effective, non-expired runtime entries and removes entries superseded
by a newer approved truth.

Each snapshot contains:

- schema version;
- deterministic Brain version hash;
- generation time;
- source path and checksum for every entry;
- authority, scope, safety, autonomy, and provenance metadata.

The Brain version is derived from canonical entry content, not the generation
time. Publishing unchanged knowledge produces the same identity.

## Retrieve governed context through MCP

The `get_business_context` MCP tool accepts a task query and agent role. It
returns only knowledge from the published snapshot, filtered by role and—by
default—`customer_safe: true`.

Every result includes memory IDs and the Brain version. Agents should record
both with their drafts and actions so decisions remain explainable and
reproducible.

```text
Business event
  → retrieve approved context
  → draft
  → evaluate facts, policy, voice, and permissions
  → approve or execute
  → record outcome with Brain version and memory IDs
```

`cerebrum publish` creates a portable artifact. Teams can bundle it with a
worker, upload it to private storage, or serve it through a controlled runtime
without exposing the rest of the development Brain.
