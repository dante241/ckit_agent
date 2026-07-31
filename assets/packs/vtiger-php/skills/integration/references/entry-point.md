# Entry Point - Vtiger_EntryPoint

## Overview

Entry points are public URLs that bypass CRM authentication. Use for webhooks, OAuth callbacks, payment gateways, and external integrations.

## Entry Point Skeleton

```php
<?php

/**
 * OAuth callback entry point
 * Location: include/EntryPoints/FacebookOAuthCallback.php
 * URL: https://crm.domain.com/entrypoint.php?name=FacebookOAuthCallback
 */

class FacebookOAuthCallback extends Vtiger_EntryPoint {

    public function process(Vtiger_Request $request) {
        try {
            // Get OAuth code from request
            $code = $request->get('code');
            $state = $request->get('state');

            if (empty($code)) {
                $this->sendError('Authorization code missing');
                return;
            }

            // Exchange code for access token
            $tokenData = $this->exchangeCodeForToken($code);

            if (empty($tokenData['access_token'])) {
                $this->sendError('Failed to get access token');
                return;
            }

            // Save token to CRM
            $this->saveAccessToken($state, $tokenData);

            // Redirect to success page
            $this->sendSuccess('Authorization successful');
        }
        catch (Exception $e) {
            error_log("OAuth callback error: " . $e->getMessage());
            $this->sendError($e->getMessage());
        }
    }

    private function exchangeCodeForToken(string $code): array {
        global $facebookConfig;

        $url = 'https://graph.facebook.com/v18.0/oauth/access_token';
        $params = [
            'client_id' => $facebookConfig['app_id'],
            'client_secret' => $facebookConfig['app_secret'],
            'redirect_uri' => $this->getCallbackUrl(),
            'code' => $code,
        ];

        $response = file_get_contents($url . '?' . http_build_query($params));
        return json_decode($response, true);
    }

    private function saveAccessToken(string $state, array $tokenData): void {
        // State contains record ID
        $recordId = (int) $state;
        if ($recordId == 0) return;

        $record = Vtiger_Record_Model::getInstanceById($recordId, 'CPAdvertisingAccount');
        $record->set('access_token', $tokenData['access_token']);
        $record->set('token_expired_date', date('Y-m-d H:i:s', time() + $tokenData['expires_in']));
        $record->set('status', 'active');
        $record->set('mode', 'edit');
        $record->save();
    }

    private function getCallbackUrl(): string {
        global $site_URL;
        return $site_URL . '/entrypoint.php?name=FacebookOAuthCallback';
    }

    private function sendError(string $message): void {
        echo "<h1>Error</h1>";
        echo "<p>" . htmlspecialchars($message) . "</p>";
        echo "<script>setTimeout(() => window.close(), 3000);</script>";
    }

    private function sendSuccess(string $message): void {
        echo "<h1>Success</h1>";
        echo "<p>" . htmlspecialchars($message) . "</p>";
        echo "<script>setTimeout(() => window.close(), 2000);</script>";
    }
}
```

## URL Format

```
https://crm.domain.com/entrypoint.php?name={ClassName}&param1=value1&param2=value2
```

Examples:
```
https://crm.domain.com/entrypoint.php?name=FacebookOAuthCallback&code=abc123&state=456
https://crm.domain.com/entrypoint.php?name=PaymentGatewayCallback&transaction_id=TX123
https://crm.domain.com/entrypoint.php?name=PublicFormSubmit&form_id=contact_us
```

## File Location

### Standard Location
```
include/EntryPoints/{ClassName}.php
```

### Custom Location
```
custom/include/EntryPoints/{ClassName}.php
```

## 22 Existing Entry Points

| Entry Point | Purpose |
|-------------|---------|
| `Download` | File downloads |
| `DownloadFile` | Module file downloads |
| `Export` | Record export |
| `ExportPDF` | PDF export |
| `GetPreview` | Document preview |
| `GetRelatedList` | Related list data |
| `Image` | Image serving |
| `ImportFile` | Import processing |
| `Login` | Custom login |
| `Logout` | Custom logout |
| `PublicForm` | Web-to-lead/contact forms |
| `Qrcode` | QR code generation |
| `RSS` | RSS feed |
| `SaveFile` | File upload |
| `SendEmail` | Email sending |
| `Site` | Public site pages |
| `SocialSharing` | Social media sharing |
| `Star` | Favorite/star records |
| `UploadFile` | File upload handler |
| `ValidateEmail` | Email validation |
| `Webhook` | Generic webhook receiver |
| `Widgets` | Dashboard widgets |

## Common Use Cases

### 1. OAuth Callback

```php
class TikTokOAuthCallback extends Vtiger_EntryPoint {

    public function process(Vtiger_Request $request) {
        $code = $request->get('code');
        $authCode = $request->get('auth_code');
        $state = $request->get('state');

        // Exchange for token
        $tokenData = $this->getAccessToken($code, $authCode);

        // Save to record
        $recordId = (int) $state;
        $record = Vtiger_Record_Model::getInstanceById($recordId);
        $record->set('access_token', $tokenData['access_token']);
        $record->set('refresh_token', $tokenData['refresh_token']);
        $record->set('mode', 'edit');
        $record->save();

        // Close window
        echo "<script>window.opener.postMessage('oauth_success', '*'); window.close();</script>";
    }
}
```

### 2. Payment Gateway Callback

```php
class PaymentGatewayCallback extends Vtiger_EntryPoint {

    public function process(Vtiger_Request $request) {
        // Verify signature
        if (!$this->verifySignature($request)) {
            http_response_code(403);
            echo json_encode(['error' => 'Invalid signature']);
            return;
        }

        $transactionId = $request->get('transaction_id');
        $status = $request->get('status');
        $amount = $request->get('amount');

        // Update payment record
        $this->updatePayment($transactionId, $status, $amount);

        // Return success
        echo json_encode(['success' => true]);
    }

    private function verifySignature(Vtiger_Request $request): bool {
        global $paymentGatewayConfig;

        $signature = $request->get('signature');
        $data = $request->get('data');

        $expectedSignature = hash_hmac('sha256', $data, $paymentGatewayConfig['secret']);

        return hash_equals($expectedSignature, $signature);
    }
}
```

### 3. Public Form Submit

```php
class PublicContactForm extends Vtiger_EntryPoint {

    public function process(Vtiger_Request $request) {
        // CORS headers
        header('Access-Control-Allow-Origin: *');
        header('Access-Control-Allow-Methods: POST');
        header('Content-Type: application/json');

        if ($request->getRequestMethod() !== 'POST') {
            http_response_code(405);
            echo json_encode(['error' => 'Method not allowed']);
            return;
        }

        // Validate reCAPTCHA
        if (!$this->validateRecaptcha($request->get('recaptcha_token'))) {
            http_response_code(400);
            echo json_encode(['error' => 'Invalid reCAPTCHA']);
            return;
        }

        // Create lead
        $leadData = [
            'lastname' => $request->get('name'),
            'email' => $request->get('email'),
            'phone' => $request->get('phone'),
            'description' => $request->get('message'),
            'leadsource' => 'Website',
            'assigned_user_id' => 1, // Admin
        ];

        $lead = Vtiger_Record_Model::getCleanInstance('Leads');
        foreach ($leadData as $field => $value) {
            $lead->set($field, $value);
        }
        $lead->save();

        echo json_encode([
            'success' => true,
            'message' => 'Thank you for contacting us!',
        ]);
    }

    private function validateRecaptcha(string $token): bool {
        global $recaptchaConfig;

        $response = file_get_contents(
            'https://www.google.com/recaptcha/api/siteverify?' . http_build_query([
                'secret' => $recaptchaConfig['secret_key'],
                'response' => $token,
            ])
        );

        $result = json_decode($response, true);
        return !empty($result['success']);
    }
}
```

## Response Patterns

### JSON Response
```php
header('Content-Type: application/json');
echo json_encode(['success' => true, 'data' => $data]);
exit();
```

### HTML Response
```php
echo "<h1>Success</h1>";
echo "<p>Your request has been processed.</p>";
exit();
```

### Redirect
```php
header('Location: https://example.com/thank-you');
exit();
```

### Close Window (OAuth)
```php
echo "<script>window.opener.postMessage({type: 'oauth_success', data: {}}, '*'); window.close();</script>";
exit();
```

## Security Considerations

**Entry points are PUBLIC** - no authentication required by default.

### Implement Security:

1. **Signature Verification:**
```php
if (!$this->verifyWebhookSignature($request)) {
    http_response_code(403);
    exit();
}
```

2. **IP Whitelist:**
```php
$allowedIPs = ['52.89.214.238', '34.212.75.30'];
if (!in_array($_SERVER['REMOTE_ADDR'], $allowedIPs)) {
    http_response_code(403);
    exit();
}
```

3. **Rate Limiting:**
```php
if ($this->isRateLimitExceeded($_SERVER['REMOTE_ADDR'])) {
    http_response_code(429);
    exit();
}
```

4. **Token Validation:**
```php
$token = $request->get('token');
if (!$this->isValidToken($token)) {
    http_response_code(401);
    exit();
}
```

## Critical Rules

1. **Bypasses auth** - Anyone can access entry point URLs
2. **Implement security** - Signature, IP whitelist, tokens, rate limiting
3. **ALWAYS exit()** - After sending response
4. **Error handling** - Try-catch and log errors
5. **CORS headers** - If called from browser
6. **HTTP methods** - Check request method if needed
7. **Custom location** - `custom/include/EntryPoints/` for custom code
