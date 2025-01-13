htmx.defineExtension('client-side-templates', {
    transformResponse: function (text, xhr, elt) {
        var mustacheTemplate = htmx.closest(elt, "[mustache-template]");
        if (mustacheTemplate) {
            // Only try to parse JSON if the response is JSON
            var contentType = xhr.getResponseHeader("Content-Type");
            if (contentType && contentType.indexOf("json") >= 0) {
                try {
                    var data = JSON.parse(text);
                    var templateId = mustacheTemplate.getAttribute('mustache-template');
                    var template = htmx.find("#" + templateId);
                    if (template) {
                        return Mustache.render(template.innerHTML, data);
                    } else {
                        throw "Unknown mustache template: " + templateId;
                    }
                } catch (e) {
                    console.error("Error processing JSON response", e);
                    return text;
                }
            }
        }
        return text;
    }
});