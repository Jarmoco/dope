/**
 * GM_* API Polyfill for InjectorProxy
 * Provides compatibility functions for userscripts that depend on ViolentMonkey/Greasemonkey APIs.
 */

(function() {
    'use strict';

    const STORAGE_PREFIX = 'injectorproxy_';
    
    // ────────────────────────────────────────────────
    // Storage Functions
    // ────────────────────────────────────────────────
    
    window.GM_setValue = function(key, value) {
        try {
            localStorage.setItem(STORAGE_PREFIX + key, JSON.stringify(value));
            return true;
        } catch (e) {
            console.error('GM_setValue error:', e);
            return false;
        }
    };
    
    window.GM_getValue = function(key, defaultValue) {
        try {
            const stored = localStorage.getItem(STORAGE_PREFIX + key);
            return stored !== null ? JSON.parse(stored) : defaultValue;
        } catch (e) {
            console.error('GM_getValue error:', e);
            return defaultValue;
        }
    };
    
    window.GM_deleteValue = function(key) {
        try {
            localStorage.removeItem(STORAGE_PREFIX + key);
            return true;
        } catch (e) {
            console.error('GM_deleteValue error:', e);
            return false;
        }
    };
    
    window.GM_listValues = function() {
        const keys = [];
        for (let i = 0; i < localStorage.length; i++) {
            const key = localStorage.key(i);
            if (key && key.startsWith(STORAGE_PREFIX)) {
                keys.push(key.substring(STORAGE_PREFIX.length));
            }
        }
        return keys;
    };
    
    // ────────────────────────────────────────────────
    // Style Injection
    // ────────────────────────────────────────────────
    
    window.GM_addStyle = function(css) {
        try {
            const style = document.createElement('style');
            style.textContent = css;
            
            // Try to add to head, fallback to document
            const target = document.head || document.documentElement;
            target.appendChild(style);
            
            return style;
        } catch (e) {
            console.error('GM_addStyle error:', e);
            return null;
        }
    };
    
// ────────────────────────────────────────────────
    // XMLHttpRequest Wrapper - Same-origin requests
    // ────────────────────────────────────────────────
    
    const REQUEST_PROXY_URL = '__injectorproxy__/request';
    
    window.GM_xmlhttpRequest = function(options) {
        // Make same-origin requests through this path to avoid CORS issues
        try {
            const xhr = new XMLHttpRequest();
            const requestUrl = options.url;

            // Check if URL is same origin
            try {
                urlToUse = requestUrl;
            } catch (e) {
                console.warn('GM_xmlhttpRequest: Invalid URL format');
                if (options.onerror) {
                    options.onerror({
                        status: 0,
                        statusText: 'Invalid URL',
                        error: e.message
                    });
                }
                if (options.onloadend) {
                    options.onloadend({
                        status: 0,
                        statusText: 'Invalid URL'
                    });
                }
                return null;
            }
            
            xhr.onreadystatechange = function() {
                if (xhr.readyState === 4) {
                    const responseHeaders = {};
                    const headerStr = xhr.getAllResponseHeaders();
                    if (headerStr) {
                        const headers = headerStr.trim().split(/[\r\n]+/);
                        headers.forEach(function(line) {
                            const parts = line.split(': ');
                            const key = parts.shift();
                            const value = parts.join(': ');
                            if (key) responseHeaders[key] = value;
                        });
                    }
                    
                    const response = {
                        response: xhr.response,
                        responseText: xhr.responseText,
                        responseXML: xhr.responseXML,
                        readyState: xhr.readyState,
                        status: xhr.status,
                        statusText: xhr.statusText,
                        finalUrl: options.url,
                        responseHeaders: responseHeaders
                    };
                    
                    if (xhr.status >= 200 && xhr.status < 300) {
                        if (options.onload) {
                            options.onload(response);
                        }
                    } else {
                        if (options.onerror) {
                            options.onerror({
                                status: xhr.status,
                                statusText: xhr.statusText,
                                error: xhr.statusText
                            });
                        }
                    }
                    
                    if (options.onloadend) {
                        options.onloadend(response);
                    }
                }
            };
            
            xhr.onprogress = function(event) {
                if (options.onprogress) {
                    options.onprogress({
                        position: event.loaded,
                        totalSize: event.total,
                        lengthComputable: event.lengthComputable
                    });
                }
            };
            
            xhr.ontimeout = function() {
                if (options.ontimeout) {
                    options.ontimeout();
                }
            };
            
            xhr.open(options.method || 'GET', urlToUse, options.async !== false);
            
            // Set headers from options
            if (options.headers) {
                for (const key in options.headers) {
                    if (options.headers.hasOwnProperty(key)) {
                        xhr.setRequestHeader(key, options.headers[key]);
                    }
                }
            }
            
            if (options.timeout) {
                xhr.timeout = options.timeout;
            }
            
            xhr.send(options.data || options.body || null);
            
            return xhr;
        } catch (e) {
            console.error('GM_xmlhttpRequest error:', e);
            
            if (options.onerror) {
                options.onerror({
                    status: 0,
                    statusText: 'Request failed',
                    error: e.message
                });
            }
            
            if (options.onloadend) {
                options.onloadend({
                    status: 0,
                    statusText: 'Request failed'
                });
            }
            
            return null;
        }
    };
    
    // ────────────────────────────────────────────────
    // Resource Management
    // ────────────────────────────────────────────────
    
    window.GM_getResourceURL = function(name) {
        console.warn('GM_getResourceURL: Resource URLs not supported in proxy mode');
        return '';
    };
    
    // ────────────────────────────────────────────────
    // Notification System
    // ────────────────────────────────────────────────
    
    window.GM_notification = function(options, ondone, onclick) {
        try {
            if (typeof Notification === 'undefined') {
                console.log('GM_notification:', options.text || options);
                if (ondone) ondone();
                return;
            }
            
            if (Notification.permission === 'granted') {
                const notification = new Notification(options.title || 'InjectorProxy', {
                    body: options.text || options,
                    icon: options.icon || '',
                    image: options.image || '',
                    tag: options.tag || ''
                });
                
                if (onclick) {
                    notification.onclick = function() {
                        onclick(notification);
                    };
                }
                
                if (ondone) {
                    notification.onclose = ondone;
                }
            } else {
                console.log('GM_notification:', options.text || options);
                if (ondone) ondone();
            }
        } catch (e) {
            console.error('GM_notification error:', e);
            if (ondone) ondone();
        }
    };
    
    // ────────────────────────────────────────────────
    // Utility Functions
    // ────────────────────────────────────────────────
    
    window.GM_registerMenuCommand = function(name, callback, accessKey) {
        console.warn('GM_registerMenuCommand: Menu commands not supported in proxy mode');
    };
    
    window.GM_unregisterMenuCommand = function(menuCmdId) {
        console.warn('GM_unregisterMenuCommand: Menu commands not supported in proxy mode');
    };
    
    window.GM_openInTab = function(url, options) {
        try {
            const newTab = window.open(url, '_blank');
            if (options && options.active === false) {
                newTab.blur();
            }
            return newTab;
        } catch (e) {
            console.error('GM_openInTab error:', e);
            return null;
        }
    };
    
    window.GM_setClipboard = function(text) {
        try {
            navigator.clipboard.writeText(text);
            return true;
        } catch (e) {
            console.error('GM_setClipboard error:', e);
            return false;
        }
    };
    
    console.log('InjectorProxy GM_* polyfill loaded successfully');
    
})();