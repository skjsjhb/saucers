#include "wkg.navigation.impl.hpp"

namespace saucer
{
    navigation::navigation(impl data) : m_impl(std::make_unique<impl>(std::move(data))) {}

    navigation::navigation(const navigation &other) : m_impl(std::make_unique<impl>(*other.m_impl)) {}

    navigation::navigation(navigation &&other) noexcept : m_impl(std::move(other.m_impl)) {}

    navigation::~navigation() = default;

    std::string navigation::url() const
    {
        auto *const action  = webkit_navigation_policy_decision_get_navigation_action(m_impl->decision.get());
        auto *const request = webkit_navigation_action_get_request(action);

        return webkit_uri_request_get_uri(request);
    }

    bool navigation::redirection() const
    {
        auto *const action = webkit_navigation_policy_decision_get_navigation_action(m_impl->decision.get());
        return webkit_navigation_action_is_redirect(action);
    }

    bool navigation::new_window() const
    {
        return m_impl->type == WEBKIT_POLICY_DECISION_TYPE_NEW_WINDOW_ACTION;
    }

    bool navigation::user_initiated() const
    {
        auto *const action = webkit_navigation_policy_decision_get_navigation_action(m_impl->decision.get());
        return webkit_navigation_action_is_user_gesture(action);
    }
} // namespace saucer
