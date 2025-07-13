#pragma once

#include <saucer/modules/module.hpp>

#include <saucer/app.hpp>
#include <saucer/window.hpp>
#include <saucer/webview.hpp>

#import <Cocoa/Cocoa.h>
#import <WebKit/WebKit.h>

namespace saucer
{
    template <>
    struct stable_natives<application>
    {
        NSApplication *application;
    };

    template <>
    struct stable_natives<window>
    {
        NSWindow *window;
    };

    template <>
    struct stable_natives<webview>
    {
        WKWebView *webview;
    };
} // namespace saucer
