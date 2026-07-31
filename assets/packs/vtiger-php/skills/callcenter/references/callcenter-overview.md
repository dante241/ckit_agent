# Call Center Overview

## Architecture

**Webhook Receiver → Connector → Process Call Data → Update CRM**

```
External Provider          VTiger System
┌─────────────┐           ┌──────────────────────┐
│   PBX/VoIP  │──webhook──│  Connector.process() │
│   Provider  │           └──────────┬───────────┘
└─────────────┘                      │
                                     ├─ Normalize phone
                                     ├─ Find Contact/Lead
                                     ├─ Create call log
                                     └─ Update records
```

## Connector Location

**Path:** `include/Webhooks/`

**Naming:** `{Provider}Connector.php`

**Parent:** Extends `Vtiger_EntryPoint` (public access, no authentication)

## Webhook Flow

### 1. External Request
Provider sends POST/GET to webhook URL:
```
POST /index.php?webhook=stringee&action=process
Content-Type: application/json

{
  "call_id": "call-123456",
  "from": "0909123456",
  "to": "0287654321",
  "status": "ended",
  "duration": 120
}
```

### 2. Connector Process
```php
class StringeeConnector extends Vtiger_EntryPoint {
    public function process(Vtiger_Request $request): void {
        // Validate webhook data
        // Parse call information
        // Normalize phone numbers
        // Find/create Contact
        // Create call log in PBXManager
        // Send response
    }
}
```

### 3. CRM Update
- Find Contact/Lead by phone number
- Create PBXManager record (call log)
- Link call to Contact
- Store recording URL
- Update call statistics

## 21 Available Connectors

### Call Center Providers (16)

| Provider | Module Name | Country | Notes |
|----------|-------------|---------|-------|
| Stringee | StringeeConnector | Vietnam | Cloud call center |
| CloudFone | CloudFoneConnector | Vietnam | VoIP provider |
| CloudCALL | CloudCALLConnector | Vietnam | Cloud PBX |
| OmiCall | OmiCallConnector | Vietnam | Call center platform |
| VoIP24H | VoIP24HConnector | Vietnam | VoIP service |
| Tel4VN | Tel4VNConnector | Vietnam | Telephony provider |
| SouthTelecom | SouthTelecomConnector | Vietnam | Telecom company |
| FPTTelecom | FPTTelecomConnector | Vietnam | FPT telecom service |
| CMCTelecom | CMCTelecomConnector | Vietnam | CMC telecom |
| FreePBX | FreePBXConnector | Global | Open source PBX |
| GrandStream | GrandStreamConnector | Global | IP PBX hardware |
| VoiceCloud | VoiceCloudConnector | Global | Cloud telephony |
| Xorcom | XorcomConnector | Global | IP PBX solutions |
| YeaStar | YeaStarConnector | Global | PBX manufacturer |
| VCS | VCSConnector | Vietnam | Voice communication |
| MiTek | MiTekConnector | Global | Call center software |

### Social/Chat Providers (5)

| Provider | Module Name | Purpose |
|----------|-------------|---------|
| Facebook | FacebookConnector | Messenger webhooks |
| Zalo | ZaloConnector | Zalo business messages |
| IndividualZalo | IndividualZaloConnector | Personal Zalo |
| Telegram | TelegramConnector | Telegram bot webhooks |
| Tawk | TawkConnector | Live chat widget |

## Common Use Cases

### Inbound Call Received
1. Provider sends webhook with call_id, from, to
2. Connector normalizes phone numbers
3. Find Contact by caller phone
4. Create PBXManager record with status "ringing"
5. Pop up call notification in CRM

### Call Ended
1. Provider sends webhook with duration, recording_url
2. Update PBXManager record with final data
3. Store recording URL
4. Update Contact last call date

### Missed Call
1. Provider sends webhook with status "missed"
2. Create PBXManager record
3. Create Task for callback

## Integration Points

- **PBXManager Module**: Stores call logs
- **Contacts Module**: Phone number lookup, link calls
- **Leads Module**: Phone number lookup for leads
- **CallCenterUtils**: Phone normalization, lookup helpers
- **Config_Model**: Provider credentials, webhook settings
