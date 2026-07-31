# Test Case Generation

> Generate structured test case documentation for VTiger components

## Test Case Document Template

```markdown
# Test Cases: {Feature Name}

**Module:** {ModuleName}
**Component:** Action / Model / API / Webhook / Event Handler
**Author:** {Name}
**Date:** {YYYY-MM-DD}

---

## TC-01: {Scenario Name}

- **Type:** Happy path / Edge case / Error case
- **Precondition:** {setup required before test}
- **Steps:**
  1. {First action}
  2. {Second action}
  3. {Third action}
- **Expected:** {Expected outcome/behavior}
- **Actual:** [ ] Pass / [ ] Fail
- **Notes:** {Additional observations}

---

## TC-02: {Next Scenario}
...
```

## Generation Rules by Component

### Action Controller Tests

Action controllers handle AJAX/JSON requests. Test areas:

1. **Happy Path**
   - Valid user with permissions
   - All required parameters provided
   - Record exists and is accessible
   - Feature is enabled

2. **Permission Checks**
   - User without module access
   - User without edit/delete permission
   - Record owned by another user

3. **Missing Parameters**
   - Missing required params (module, record, etc.)
   - Empty string parameters
   - Null parameters

4. **Invalid Record**
   - Non-existent record ID
   - Deleted record
   - Record from wrong module

5. **Duplicate Prevention**
   - Attempt to create duplicate record
   - Unique field validation

6. **CSRF Protection**
   - Request without CSRF token
   - Invalid CSRF token

7. **Feature Gate**
   - Feature disabled in config
   - Feature disabled for user role

**Example:**
```markdown
## TC-01: Save Account - Happy Path
- **Type:** Happy path
- **Precondition:** User logged in with Accounts edit permission
- **Steps:**
  1. POST to index.php?module=Accounts&action=Save
  2. Provide valid accountname, assigned_user_id
  3. Include valid CSRF token
- **Expected:** Record saved, returns success JSON with record ID
- **Actual:** [ ] Pass / [ ] Fail
```

### Helper/Model Tests

Helper and Model classes contain business logic. Test areas:

1. **Normal Input**
   - Standard valid data
   - Typical use case scenarios

2. **Empty Input**
   - Empty string
   - Null values
   - Empty arrays

3. **Boundary Cases**
   - Maximum length strings
   - Very large numbers
   - Date boundaries (leap year, end of month)
   - Zero values

4. **SQL Injection**
   - Input with SQL keywords (SELECT, DROP, etc.)
   - Single quotes, double quotes
   - Comment syntax (-- , /* */)

5. **XSS Prevention**
   - Script tags in text
   - Event handlers (onclick, onerror)
   - Data URIs

6. **Vietnamese Text**
   - Unicode Vietnamese characters (ăâêôơưđ)
   - Tone marks
   - Special characters

7. **Database Error Handling**
   - Simulate DB connection failure
   - Invalid query syntax
   - Constraint violations

**Example:**
```markdown
## TC-03: Calculate Goal Progress - Boundary Case
- **Type:** Edge case
- **Precondition:** Goal record with target = 0
- **Steps:**
  1. Call calculateProgress($goalId)
  2. Provide goal with actual = 100, target = 0
- **Expected:** Return 0% or handle division by zero gracefully
- **Actual:** [ ] Pass / [ ] Fail
```

### API Handler Tests

API handlers process external integration requests. Test areas:

1. **Authentication**
   - No token provided
   - Invalid token format
   - Expired token
   - Valid token

2. **Feature Gate**
   - Integration disabled in config
   - Integration not enabled for account

3. **CRUD Operations**
   - Create record
   - Update existing record
   - Retrieve record
   - Delete record

4. **Validation**
   - Required fields missing
   - Invalid data types
   - Field value constraints

5. **Error Handling**
   - Database errors
   - External API failures
   - Malformed JSON

6. **Idempotency**
   - Duplicate create requests
   - Multiple update requests

**Example:**
```markdown
## TC-05: Zalo API - No Token
- **Type:** Error case
- **Precondition:** Zalo integration enabled
- **Steps:**
  1. POST to api/IntegrationAPI/Zalo/CreateOrder
  2. Do not include Authorization header
- **Expected:** HTTP 401, JSON error "Missing authentication token"
- **Actual:** [ ] Pass / [ ] Fail
```

### Webhook Connector Tests

Webhook connectors receive external HTTP requests. Test areas:

1. **Valid Payload**
   - Standard webhook payload
   - All required fields present

2. **Malformed Payload**
   - Invalid JSON syntax
   - Missing required fields
   - Wrong data types

3. **Duplicate Event**
   - Same event ID received twice
   - Idempotency handling

4. **Authentication**
   - Valid signature/token
   - Invalid signature
   - Missing auth header

5. **Payload Variations**
   - Minimal required fields
   - Full payload with optional fields
   - Unknown/extra fields

**Example:**
```markdown
## TC-07: Webhook - Malformed JSON
- **Type:** Error case
- **Precondition:** Webhook endpoint is active
- **Steps:**
  1. POST to webhooks/Zalo.php
  2. Send invalid JSON (missing closing brace)
- **Expected:** HTTP 400, log error, no record created
- **Actual:** [ ] Pass / [ ] Fail
```

### Event Handler Tests

Event handlers trigger on CRM events (save, delete, link). Test areas:

1. **afterSave - Create Mode**
   - New record creation
   - All fields populated
   - Related records created

2. **afterSave - Edit Mode**
   - Field value changed
   - Field value unchanged
   - Related record updated

3. **Field Change Detection**
   - Specific field changed (status, amount, etc.)
   - Multiple fields changed
   - No fields changed

4. **Related Record Operations**
   - Create related record
   - Update related record
   - Link to existing record

5. **beforeDelete**
   - Cleanup related records
   - Prevent deletion with validation

6. **Workflow Integration**
   - Handler executes before workflow
   - Handler executes after workflow

**Example:**
```markdown
## TC-09: Goal Handler - Status Change
- **Type:** Happy path
- **Precondition:** Goal record exists with status = "In Progress"
- **Steps:**
  1. Update goal status to "Completed"
  2. Save record
  3. AfterSave handler executes
- **Expected:** Completion date set to now, notification sent
- **Actual:** [ ] Pass / [ ] Fail
```

## Test Case Prioritization

**P0 - Critical (Must Test)**
- Happy path for core functionality
- Security validations (SQL injection, XSS, auth)
- Data integrity (save, delete, relations)

**P1 - High (Should Test)**
- Error handling
- Edge cases (empty, null, boundary)
- Feature gates and permissions

**P2 - Medium (Nice to Test)**
- Vietnamese text handling
- Performance with large datasets
- UI/UX error messages

**P3 - Low (Optional)**
- Rare edge cases
- Backward compatibility
- Legacy code paths
