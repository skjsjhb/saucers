#include "modules/stable/qt.hpp"

#include "qt.app.impl.hpp"
#include "qt.window.impl.hpp"
#include "qt.webview.impl.hpp"

namespace saucer
{
    template <>
    natives<application, true> application::native<true>() const
    {
        return {.application = m_impl->application.get()};
    }

    template <>
    natives<window, true> window::native<true>() const
    {
        return {.window = m_impl->window.get()};
    }

    template <>
    natives<webview, true> webview::native<true>() const
    {
        return {.webview = m_impl->web_view.get()};
    }
} // namespace saucer
