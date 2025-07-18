#include "cocoa.app.impl.hpp"

namespace saucer
{
    application::application(const options &opts) : extensible(this), m_pool(opts.threads), m_impl(std::make_unique<impl>())
    {
        m_impl->thread      = std::this_thread::get_id();
        m_impl->application = [NSApplication sharedApplication];

        [NSApp activateIgnoringOtherApps:YES];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

        impl::init_menu();
    }

    application::~application() = default;

    bool application::thread_safe() const
    {
        return m_impl->thread == std::this_thread::get_id();
    }

    void application::post(callback_t callback) const // NOLINT(*-static)
    {
        auto *const queue = dispatch_get_main_queue();
        auto *const ptr   = new callback_t{std::move(callback)};

        dispatch_async(queue,
                       [ptr]
                       {
                           const utils::autorelease_guard guard{};

                           auto callback = std::unique_ptr<callback_t>{ptr};
                           std::invoke(*callback);
                       });
    }

    template <>
    void application::run<true>() const // NOLINT(*-static)
    {
        [NSApp run];
    }

    template <>
    void application::run<false>() const // NOLINT(*-static)
    {
        const utils::autorelease_guard guard{};

        auto *const event = [NSApp nextEventMatchingMask:NSEventMaskAny
                                               untilDate:[NSDate now]
                                                  inMode:NSDefaultRunLoopMode
                                                 dequeue:YES];

        if (!event)
        {
            return;
        }

        [NSApp sendEvent:event];
    }

    void application::quit() // NOLINT(*-static)
    {
        [NSApp stop:nil];
        [NSApp postEvent:[NSEvent otherEventWithType:NSEventTypeApplicationDefined
                                        location:NSZeroPoint
                                   modifierFlags:0
                                       timestamp:0
                                    windowNumber:0
                                         context:nil
                                         subtype:0
                                           data1:0
                                           data2:0]
             atStart:YES];
    }
} // namespace saucer
