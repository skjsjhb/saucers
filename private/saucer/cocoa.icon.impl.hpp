#pragma once

#include "icon.hpp"

#include "cocoa.utils.hpp"

#import <Cocoa/Cocoa.h>

namespace saucer
{
    struct icon::impl
    {
        utils::objc_ptr<NSImage> icon;
    };
} // namespace saucer
