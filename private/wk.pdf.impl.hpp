#pragma once

#include "pdf.hpp"

#import <Cocoa/Cocoa.h>

#include <functional>

@class PrintDelegate;

@interface PrintDelegate : NSObject
{
  @public
    std::function<void()> m_callback;
}
- (instancetype)initWithCallback:(std::function<void()>)callback;
- (void)printOperationDidRun:(NSPrintOperation *)printOperation success:(BOOL)success contextInfo:(void *)contextInfo;
@end
