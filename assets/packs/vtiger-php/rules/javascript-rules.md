---
description: "jQuery/JS conventions — header, app.request, không inline JS. Auto-load (TTSR) khi sửa file khớp."
scope: "tool:edit(**/*.js), tool:write(**/*.js)"
condition: ".*"
---

# jQuery / JavaScript Conventions

> Loads only when editing JS files.

## File Header

```javascript
/*
    FileName.js
    Author: Your Name
    Date: YYYY-MM-DD
    Purpose: Brief description
*/
```

## Controller Pattern: `Vtiger.Class()`

```javascript
CustomView_BaseController_Js('ModuleName_ViewName_Js', {}, {

    registerEvents: function() {
        this._super();
        var self = this;
        var container = this.getContainer();  // Cache DOM

        container.find('#btnSave').on('click', function() {
            self.handleSave();
            return false;
        });
    }
});
```

**Controller naming:** `<Module>_<View>_Js`. Parent classes: `Vtiger_List_Js`, `Vtiger_Edit_Js`, `Vtiger_Detail_Js`, `CustomView_BaseController_Js`.

```javascript
// Custom views — extend CustomView_BaseController_Js
CustomView_BaseController_Js('Products_CheckWarranty_Js', {}, { });

// Core views — extend matching Vtiger parent controller
Vtiger_List_Js('Accounts_List_Js', {}, { });
Vtiger_Edit_Js('Contacts_Edit_Js', {}, { });
Vtiger_Detail_Js('Accounts_Detail_Js', {}, { });
```

## DOM Selection Caching

Cache jQuery selectors at the top of `registerEvents()` — never re-query inside loops/callbacks:

```javascript
registerEvents: function() {
    var self = this;
    var container = this.getContainer();   // Cache root
    var form = container.find('form');     // Cache form
    var btnSubmit = form.find('#btnSubmit');

    btnSubmit.on('click', function() {
        self.handleSubmit(form);
    });
}
```

## File Location

| View Type | Location |
|-----------|----------|
| Core views (List, Edit, Detail — inherit parent controller) | `layouts/v7/modules/<Module>/resources/<View>.js` |
| Custom views (Config, Report, custom pages — standalone) | `modules/<Module>/resources/<View>.js` |

## AJAX Pattern: `app.request.post()`

```javascript
app.helper.showProgress();
var params = { module: 'Products', action: 'SaveAjax', record: recordId };

app.request.post({ data: params }).then(function(error, data) {
    app.helper.hideProgress();

    if (error) {
        app.helper.showErrorNotification({ message: app.vtranslate('JS_ERROR_OCCURRED') });
        return;
    }

    app.helper.showSuccessNotification({ message: app.vtranslate('JS_SAVED_SUCCESSFULLY') });
});
```

## Key JavaScript Objects

| Object | Methods | Purpose |
|--------|---------|---------|
| `app.request` | `post()`, `get()` | AJAX requests |
| `app.helper` | `showProgress()`, `hideProgress()`, `showSuccessNotification()`, `showErrorNotification()`, `showModal()`, `hideModal()` | UI helpers |
| `app` | `vtranslate(key)` | JS translation |

## Vue.js Data Properties

Use snake_case for Vue.js data fields to match backend payload keys:

```javascript
data: {
    last_campaign_id: null,
    last_campaign_url: '',
    last_campaign_name: '',
}
```

### Conditional Data Assignment (Vue.js)

```javascript
if (this.active_customer_profile.last_campaign_id) {
    customerProfileForm.related_campaign       = this.active_customer_profile.last_campaign_id;
    customerProfileForm.related_campaign_label = this.active_customer_profile.last_campaign_name;
}
```

## Comments & Modification Tracking

Same comment + attribution conventions as `php-conventions.md` (§ Modification Tracking Comments) — applied identically to JS:
- **Attribution by ownership.** New function in a file someone else owns → `// Added by <Name> on <DATE> - <REASON>`; editing someone else's function → `// Modified by <Name> on <DATE> - <REASON>`; your own file/function → plain description or none. Close a named block with `// End <Name>`. Owner = file's `Author:` header; no header → resolve via `git blame`, never assume you own it.
- **`<REASON>` in English**, regardless of the file's comment language.
- **≤ 2 lines per comment block; comment the meaning/behaviour** — not chat decisions/tradeoffs, and NO ticket / plan / phase refs.

```javascript
// Added by Nguyen Tung on 2026-03-06 - cancel link must not submit the form
container.find('.cancelLink').on('click', function (e) {
    e.preventDefault();
});
// End Nguyen Tung
```

## Security

- Use `.text()` not `.html()` for dynamic content (prevent XSS)
- Cache DOM with `container = this.getContainer()`
- Match existing line endings (`\r\n` in some files)
- Tabs for indentation
