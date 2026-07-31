---
name: cron
description: "VTiger cron/queue — cron service, vtiger_cron_task, supervisord, batch background. Use when: tạo job định kỳ, chạy nền, queue, đồng bộ theo lịch; keywords: cron, scheduler, background, batch."
user-invocable: false
---

# VTiger Cron & Queue Processing

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

This skill applies when implementing:
- Scheduled periodic tasks (data sync, cleanup, calculations)
- Background queue processing (notifications, webhooks, batch operations)
- Real-time event processing with supervisord
- Batch data transformations or imports

## ASK USER FIRST

Before implementation, determine the appropriate pattern:

**Questions to ask:**
1. Is this a periodic task (hourly/daily) or continuous processing?
2. Does it need real-time execution or can it wait for next schedule?
3. What is the expected record volume per execution?
4. Does it require external API calls or heavy computation?

## Decision Table: Cron vs Queue

| Pattern | Use When | Frequency | CPU Usage | Examples |
|---------|----------|-----------|-----------|----------|
| **Cron** | Periodic schedules | 1min - 1day | Low-Medium | Data sync, cleanup, reports |
| **Queue** | Continuous processing | Real-time | Medium-High | Notifications, webhooks, chat |

**Cron Pattern:**
- Service file: `cron/modules/<Module>/<Name>.service`
- Logic class: `modules/<Module>/crons/<name>.php`
- Runs via vtigercron.php on schedule

**Queue Pattern:**
- Service file: `cron/modules/<Module>/<Name>.service`
- Process class: `modules/<Module>/services/<ProcessName>.php`
- Extends BaseProcess, runs continuously via supervisord
- Uses CPQueue module for task management

## File Locations

```
cron/modules/<Module>/
├── TaskName.service              # Thin wrapper (3-5 lines)

modules/<Module>/
├── crons/
│   └── taskName.php              # Cron logic class
└── services/
    └── ProcessName.php           # Queue process class
```

## Registration

Tasks must be registered in `vtiger_cron_task` table:

```php
// In migration file
$this->createCronjob(
    'TaskName',                    // Name
    3600,                          // Frequency in seconds
    'Module',                      // Module name
    'cron/modules/Module/TaskName.service',  // Handler path
    'Cronjob'                      // run_by: 'Cronjob' or 'Supervisor'
);
```

## Critical Pitfalls

1. **Service File Bloat**: Keep service files 3-5 lines only, all logic in class
2. **Missing Error Isolation**: Wrap each record in try-catch to prevent cascade failures
3. **Memory Leaks**: Use batch processing for large datasets, clear variables
4. **Missing decodeUTF8()**: Always decode fetchByAssoc() results
5. **Direct SQL Concatenation**: Use pquery() with params, never concatenate
6. **Missing Supervisor Config**: Queue processes need supervisord configuration
7. **No Batch Limit**: Queue processes must limit batch size (default 200)
8. **CPU Spike**: Queue processes must sleep(2) between batches

## Architecture

**Cron Pattern (2 files):**
- Thin service wrapper calls logic class
- Logic class: `{Module}_{Name}_Cron` naming
- Method: `process()` for execution
- Error isolation per record

**Queue Pattern (2 files):**
- Thin service wrapper instantiates process
- Process extends BaseProcess
- Framework handles queue polling, batch limits, sleep
- Methods: `beforeExecuteTasks()`, `afterExecuteTasks()`

## References

- [Cron Pattern](references/cron-pattern.md) - Periodic scheduled tasks
- [Queue Pattern](references/queue-pattern.md) - Continuous queue processing
- [Registration](references/registration.md) - Task registration and configuration

## Common Frequencies

| Interval | Seconds | Use Case |
|----------|---------|----------|
| 1 minute | 60 | Real-time monitoring |
| 5 minutes | 300 | Frequent sync |
| 15 minutes | 900 | Regular updates |
| 1 hour | 3600 | Hourly reports |
| 1 day | 86400 | Daily cleanup |

## Testing

```bash
# Run all cron tasks
php cron/vtigercron.php

# Run specific task
php cron/vtigercron.php -m "TaskName"
```

## Next Steps

1. Determine pattern (Cron vs Queue) by asking user
2. Create service wrapper file
3. Create logic/process class
4. Create migration for registration
5. Test manually before deploying

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Cron service chuẩn: `cron/modules/CPSocialIntegration/SyncZaloGroupInfo.service`
- Migration đăng ký cron: `modules/CPMigration/migrations/2026.06.22.12.35.00_SyncZaloGroupInfoCron.php`

## Verify

```bash
# Chạy service trực tiếp 1 lần, xem output + exit code:
php vtigercron.php <ServiceName>   # hoặc invoke file .service theo pattern module
mysql <db> -e "SELECT name,status,laststart,lastend FROM vtiger_cron_task WHERE name LIKE '%<Service>%'"
```
