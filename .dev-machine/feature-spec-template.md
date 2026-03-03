# Feature Spec Template

> Use this template when writing feature specs for the development machine.
> Every field must be filled in. The machine implements specs literally.

---

# F-XXX: [Feature Name]

## 1. Overview

**Module:** [which module this feature belongs to]
**Priority:** [P0/P1/P2]
**Depends on:** [list feature IDs this depends on, or "none"]

[2-3 sentences: what this feature does and why it's needed]

## 2. Requirements

### Interfaces

```
[Rust signatures for any new or modified interfaces]
[Include types, function signatures, struct/trait definitions]
```

### Behavior

- [Concrete behavior rule 1]
- [Concrete behavior rule 2]
- [API responses, state transitions, etc.]

## 3. Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-1 | [Specific, testable criterion] | [How to verify: unit test, integration test, manual check] |
| AC-2 | [Specific, testable criterion] | [How to verify] |
| AC-3 | [Specific, testable criterion] | [How to verify] |

## 4. Edge Cases

| Case | Expected Behavior |
|------|-------------------|
| [Edge case 1] | [What should happen] |
| [Edge case 2] | [What should happen] |

## 5. Files to Create/Modify

| File | Action | Contents |
|------|--------|----------|
| `src/path/to/new-file.rs` | Create | [Description of what this file contains] |
| `src/path/to/existing.rs` | Modify | [Description of changes] |
| `tests/path/to/test.rs` | Create | [Tests for this feature] |

## 6. Dependencies

- [Crate dependencies needed]
- [Or "No new dependencies"]

## 7. Python Source Reference

- **Python source:** `/Users/michaeljabbour/dev/amplifier-foundation/amplifier_foundation/<module>.py`
- **Python tests:** `/Users/michaeljabbour/dev/amplifier-foundation/tests/test_<module>.py`
- [Note any Python behaviors that need special attention in Rust translation]

## 8. Notes

- [Implementation caveats]
- [Future work deferred]
- [Warnings about gotchas]
