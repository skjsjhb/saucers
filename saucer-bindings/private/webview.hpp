#pragma once

#include "webview.h"

#include <saucer/webview.hpp>

struct saucer_handle : saucer::webview
{
    saucer_on_message m_on_message{};
    saucer_on_message_with_arg m_on_message_with_arg{};
    void *m_on_message_arg = nullptr;

public:
    using saucer::webview::webview;

  public:
    bool on_message(const std::string &message) override
    {
        if (saucer::webview::on_message(message))
        {
            return true;
        }

        if (m_on_message) {
            return m_on_message(message.c_str());
        }

        if (m_on_message_with_arg && m_on_message_arg) {
            return m_on_message_with_arg(message.c_str(), m_on_message_arg);
        }

        return false;
    }
};
