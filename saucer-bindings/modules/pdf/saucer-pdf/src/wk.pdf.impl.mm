#include "wk.pdf.impl.hpp"

@implementation PrintDelegate
- (instancetype)initWithCallback:(std::function<void()>)callback
{
    self             = [super init];
    self->m_callback = std::move(callback);

    return self;
}
- (void)printOperationDidRun:(NSPrintOperation *)printOperation success:(BOOL)success contextInfo:(void *)contextInfo
{
    std::invoke(m_callback);
}
@end
