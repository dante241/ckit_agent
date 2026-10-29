---
name: database
description: "VTiger database — PearDatabase pquery, vtiger_crmentity, JOIN deleted=0, transaction, datetime GMT0. Use when: viết SQL, query, truy vấn, bảng, index, N+1, tối ưu query; schema change → skill migration."
user-invocable: false
---

# VTiger Database Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

Activate this skill when:
- Writing SQL queries for VTiger
- Performing database operations (SELECT, INSERT, UPDATE, DELETE)
- Working with VTiger table structures and relationships
- Handling datetime conversions between DB and user formats
- Optimizing database performance
- Implementing transactions
- Working with vtiger_crmentity and module tables

## Golden Rules

1. **DB Instance**: `global $adb;` hoặc `PearDatabase::getInstance()` — cả hai cùng trả singleton; theo pattern file đang sửa
2. **Parameterized Queries**: ALWAYS use `pquery()` with params array (prevents SQL injection)
3. **UTF-8 Decoding**: ALWAYS call `decodeUTF8()` on `fetchByAssoc()` results
4. **Soft Delete / N+1 / index**: theo `rule://security-rules` + `rule://performance-rules` (JOIN `vtiger_crmentity` luôn kèm `deleted = 0`)
5. **Transactions**: `startTransaction()` … `completeTransaction()`; KHÔNG có `rollbackTransaction()` — muốn rollback thì `$adb->database->FailTrans()` trước `completeTransaction()` (lỗi SQL trong transaction cũng tự đánh dấu fail)
6. **Deprecated — KHÔNG dùng** (nguồn DevKit): `query()` (SQL injection), `limitQuery()` (dùng pquery + LIMIT x,y). INSERT vào bảng dùng crmid-style id KHÔNG tự tăng → lấy id bằng `$adb->getUniqueID('<table>')` trước khi INSERT (bảng AUTO_INCREMENT thường thì `getLastInsertID()`).
7. **DB access qua layer**: KHÔNG query trực tiếp trong Action/View/EntryPoint — đi qua Data helper / Model / Utils (xem skill `action` mục Controller Mỏng).

## Quick Reference: PearDatabase Methods

```php
// Execute query with params
$result = $adb->pquery($sql, $params);

// Get single scalar value
$count = $adb->getOne($sql, $params);

// Fetch single row as associative array
$row = $adb->fetchByAssoc($result);
$row = decodeUTF8($row);

// Get row count
$numRows = $adb->num_rows($result);

// Build a literal INSERT (values already quoted, NO placeholders) — run with pquery($sql) without params
$sql = $adb->sql_insert_data($tableName, $dataArray);

// Get affected rows (UPDATE/DELETE)
$affected = $adb->getAffectedRowCount($result);

// Transactions (no rollbackTransaction(); FailTrans() forces rollback on complete)
$adb->startTransaction();
$adb->database->FailTrans();
$failed = $adb->hasFailedTransaction(); // only meaningful before completeTransaction()
$adb->completeTransaction();

// Get last inserted ID
$id = $adb->getLastInsertID();

// Generate IN clause placeholders
$marks = generateQuestionMarks($values);
```

## Critical Pitfalls

1. **Missing deleted=0 filter**: Queries return soft-deleted records
2. **SQL concatenation**: Opens SQL injection vulnerabilities
3. **Skipping decodeUTF8()**: UTF-8 data corruption in non-ASCII characters
4. **Direct column functions in WHERE**: Breaks index usage (`WHERE YEAR(date)` vs `WHERE date >= ?`)
5. **SELECT * in production**: Performance degradation, unnecessary data transfer
6. **Unclosed transactions**: Database locks and deadlocks

## Reference Files

- [Database Operations](references/database.md) - Core DB patterns, queries, transactions
- [Database Structure](references/db-structure.md) - Table architecture, naming conventions
- [Relationships](references/relationships.md) - 1:1, 1:N, N:N relationships
- [DateTime Handling](references/datetime.md) - DB↔User format conversion, timezones
- [Performance Optimization](references/performance.md) - Indexing, query optimization, caching

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Helper Data (SQL/pquery chuẩn, tung.nguyen 7/7): `modules/CPMasterPlan/helpers/Data.php`
- Helper Logic (tung.nguyen 5/5): `modules/CPMasterPlan/helpers/Logic.php`
- List không N+1: dựng record model từ row đã query bằng `$moduleModel->getRecordFromArray($row)` (`modules/Vtiger/models/Module.php:399`, dùng ở `modules/Vtiger/models/ListView.php:315`) thay vì `getInstanceById()` trong vòng lặp

## Verify

```bash
# Query mới: chạy thử trên DB thật TRƯỚC khi nhúng vào code (EXPLAIN nếu bảng lớn)
mysql <db> -e "EXPLAIN <SQL với giá trị mẫu>"
# Sau code: php -l + smoke endpoint/page dùng query đó
```
