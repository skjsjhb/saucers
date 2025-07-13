#include "qt.webview.impl.hpp"

#include "instantiate.hpp"
#include "qt.icon.impl.hpp"
#include "qt.window.impl.hpp"

#include <fmt/core.h>
#include <fmt/xchar.h>

#include <QWebEngineScriptCollection>
#include <QWebEngineProfile>
#include <QWebEngineSettings>
#include <QWebEngineUrlScheme>

namespace saucer
{
    webview::webview(const preferences &prefs) : window(prefs), extensible(this), m_impl(std::make_unique<impl>())
    {
        static std::once_flag flag;
        std::call_once(flag, [] { register_scheme("saucer"); });

        auto flags = prefs.browser_flags;

        if (prefs.hardware_acceleration)
        {
            flags.emplace("--enable-oop-rasterization");
            flags.emplace("--enable-gpu-rasterization");

            flags.emplace("--use-gl=desktop");
            flags.emplace("--enable-native-gpu-memory-buffers");
        }

        const auto args = fmt::format("{}", fmt::join(flags, " "));
        qputenv("QTWEBENGINE_CHROMIUM_FLAGS", args.c_str());

        m_impl->profile = std::make_unique<QWebEngineProfile>("saucer");

        if (!prefs.user_agent.empty())
        {
            m_impl->profile->setHttpUserAgent(QString::fromStdString(prefs.user_agent));
        }

        if (!prefs.storage_path.empty())
        {
            const auto path = QString::fromStdString(prefs.storage_path.string());

            m_impl->profile->setCachePath(path);
            m_impl->profile->setPersistentStoragePath(path);
        }

        m_impl->profile->setPersistentCookiesPolicy(prefs.persistent_cookies ? QWebEngineProfile::ForcePersistentCookies
                                                                             : QWebEngineProfile::NoPersistentCookies);

        m_impl->profile->settings()->setAttribute(QWebEngineSettings::LocalContentCanAccessRemoteUrls, true);

        m_impl->web_view    = std::make_unique<QWebEngineView>();
        m_impl->web_page    = std::make_unique<QWebEnginePage>(m_impl->profile.get());
        m_impl->channel     = std::make_unique<QWebChannel>();
        m_impl->channel_obj = std::make_unique<impl::web_class>(this);

        window::m_impl->window->setCentralWidget(m_impl->web_view.get());

        m_impl->web_view->setPage(m_impl->web_page.get());
        m_impl->web_page->setWebChannel(m_impl->channel.get());
        m_impl->channel->registerObject("saucer", m_impl->channel_obj.get());

        m_impl->web_view->connect(m_impl->web_view.get(), &QWebEngineView::loadStarted,
                                  [this]
                                  {
                                      m_impl->dom_loaded = false;
                                      m_events.at<web_event::load>().fire(state::started);
                                  });

        window::m_impl->on_closed = [this]
        {
            set_dev_tools(false);
        };

        inject({.code = impl::inject_script(), .time = load_time::creation, .permanent = true});
        inject({.code = std::string{impl::ready_script}, .time = load_time::ready, .permanent = true});

        m_impl->web_view->show();
    }

    webview::~webview()
    {
        std::invoke(window::m_impl->on_closed);
        window::m_impl->on_closed = {};

        m_impl->web_view->disconnect();
    }

    icon webview::favicon() const
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return favicon(); });
        }

        return {{m_impl->web_view->icon()}};
    }

    std::string webview::page_title() const
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return page_title(); });
        }

        return m_impl->web_view->title().toStdString();
    }

    bool webview::dev_tools() const
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return dev_tools(); });
        }

        return m_impl->dev_page != nullptr;
    }

    std::string webview::url() const
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return url(); });
        }

        return m_impl->web_view->url().toString().toStdString();
    }

    bool webview::context_menu() const
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return context_menu(); });
        }

        return m_impl->web_view->contextMenuPolicy() == Qt::ContextMenuPolicy::DefaultContextMenu;
    }

    color webview::background() const
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return background(); });
        }

        const auto color = m_impl->web_page->backgroundColor();

        return {
            static_cast<std::uint8_t>(color.red()),
            static_cast<std::uint8_t>(color.green()),
            static_cast<std::uint8_t>(color.blue()),
            static_cast<std::uint8_t>(color.alpha()),
        };
    }

    bool webview::force_dark_mode() const
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return force_dark_mode(); });
        }

#ifdef SAUCER_QT6
        const auto *settings = m_impl->profile->settings();
        return settings->testAttribute(QWebEngineSettings::ForceDarkMode);
#else
        return false;
#endif
    }

    void webview::set_dev_tools(bool enabled)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, enabled] { return set_dev_tools(enabled); });
        }

        if (!m_impl->dev_page && !enabled)
        {
            return;
        }

        if (!enabled)
        {
            m_impl->web_page->setDevToolsPage(nullptr);
            m_impl->dev_page.reset();

            return;
        }

        if (!m_impl->dev_page)
        {
            m_impl->dev_page = std::make_unique<QWebEngineView>();
            m_impl->web_page->setDevToolsPage(m_impl->dev_page->page());
        }

        m_impl->dev_page->show();
    }

    void webview::set_context_menu(bool enabled)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, enabled] { return set_context_menu(enabled); });
        }

        m_impl->web_view->setContextMenuPolicy(enabled ? Qt::ContextMenuPolicy::DefaultContextMenu
                                                       : Qt::ContextMenuPolicy::NoContextMenu);
    }

    void webview::set_background(const color &color)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, color] { return set_background(color); });
        }

        const auto [r, g, b, a] = color;
        const auto transparent  = a < 255;

        m_impl->web_view->setAttribute(Qt::WA_TranslucentBackground, transparent);
        window::m_impl->set_alpha(transparent ? 0 : 255);

        m_impl->web_page->setBackgroundColor({r, g, b, a});
    }

    void webview::set_force_dark_mode(bool enabled)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, enabled] { return set_force_dark_mode(enabled); });
        }

#ifdef SAUCER_QT6
        auto *settings = m_impl->profile->settings();
        settings->setAttribute(QWebEngineSettings::ForceDarkMode, enabled);
#endif
    }

    void webview::set_file(const fs::path &file)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, file] { return set_file(file); });
        }

        m_impl->web_view->setUrl(QUrl::fromLocalFile(QString::fromStdString(file.string())));
    }

    void webview::set_url(const std::string &url)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, url] { return set_url(url); });
        }

        m_impl->web_view->setUrl(QString::fromStdString(url));
    }

    void webview::back()
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return back(); });
        }

        m_impl->web_view->back();
    }

    void webview::forward()
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return forward(); });
        }

        m_impl->web_view->forward();
    }

    void webview::reload()
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return reload(); });
        }

        m_impl->web_view->reload();
    }

    void webview::clear_scripts()
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this] { return clear_scripts(); });
        }

        m_impl->web_view->page()->scripts().clear();

        for (const auto &script : m_impl->permanent_scripts)
        {
            inject(script);
        }
    }

    void webview::execute(const std::string &code)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, code] { execute(code); });
        }

        if (!m_impl->dom_loaded)
        {
            m_impl->pending.emplace_back(code);
            return;
        }

        m_impl->web_view->page()->runJavaScript(QString::fromStdString(code));
    }

    void webview::handle_scheme(const std::string &name, scheme::resolver &&resolver, launch policy)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, name, handler = std::move(resolver), policy] mutable
                                      { return handle_scheme(name, std::move(handler), policy); });
        }

        if (m_impl->schemes.contains(name))
        {
            return;
        }

        auto handler = scheme::handler{m_parent.get(), policy, std::move(resolver)};
        auto &scheme = m_impl->schemes.emplace(name, std::move(handler)).first->second;

        m_impl->web_view->page()->profile()->installUrlSchemeHandler(QByteArray::fromStdString(name), &scheme);
    }

    void webview::remove_scheme(const std::string &name)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, name] { return remove_scheme(name); });
        }

        const auto it = m_impl->schemes.find(name);

        if (it == m_impl->schemes.end())
        {
            return;
        }

        m_impl->web_view->page()->profile()->removeUrlSchemeHandler(&it->second);
        m_impl->schemes.erase(it);
    }

    void webview::clear(web_event event)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, event] { return clear(event); });
        }

        m_events.clear(event);
    }

    void webview::remove(web_event event, std::uint64_t id)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, event, id] { return remove(event, id); });
        }

        m_events.remove(event, id);
    }

    template <web_event Event>
    void webview::once(events::type<Event> callback)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, callback = std::move(callback)] mutable
                                      { return once<Event>(std::move(callback)); });
        }

        m_impl->setup<Event>(this);
        m_events.at<Event>().once(std::move(callback));
    }

    template <web_event Event>
    std::uint64_t webview::on(events::type<Event> callback)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, callback = std::move(callback)] mutable
                                      { return on<Event>(std::move(callback)); });
        }

        m_impl->setup<Event>(this);
        return m_events.at<Event>().add(std::move(callback));
    }

    void webview::register_scheme(const std::string &name)
    {
        auto scheme = QWebEngineUrlScheme{name.c_str()};
        scheme.setSyntax(QWebEngineUrlScheme::Syntax::Host);

        using enum QWebEngineUrlScheme::Flag;

#ifdef SAUCER_QT6
        scheme.setFlags(SecureScheme | FetchApiAllowed | CorsEnabled);
#else
        scheme.setFlags(SecureScheme | CorsEnabled);
#endif

        QWebEngineUrlScheme::registerScheme(scheme);
    }

    SAUCER_INSTANTIATE_EVENTS(6, webview, web_event);
} // namespace saucer
