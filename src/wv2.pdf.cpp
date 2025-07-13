#include "pdf.hpp"

#include "wv2.webview.impl.hpp"

#include <atomic>

namespace saucer::modules
{
    void pdf::save(const print_settings &settings)
    {
        if (!m_parent->parent().thread_safe())
        {
            return m_parent->parent().dispatch([this, settings] { return save(settings); });
        }

        ComPtr<ICoreWebView2_7> webview;
        ComPtr<ICoreWebView2Environment6> environment;

        if (!SUCCEEDED(m_parent->native<false>()->web_view.As(&webview)))
        {
            return;
        }

        if (ComPtr<ICoreWebView2Environment> env;
            !SUCCEEDED(webview->get_Environment(&env)) || !SUCCEEDED(env.As(&environment)))
        {
            return;
        }

        ComPtr<ICoreWebView2PrintSettings> print_settings;

        if (!SUCCEEDED(environment->CreatePrintSettings(&print_settings)))
        {
            return;
        }

        auto [width, height] = settings.size;
        auto orientation     = settings.orientation == layout::landscape ? COREWEBVIEW2_PRINT_ORIENTATION_LANDSCAPE
                                                                         : COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT;

        print_settings->put_PageWidth(width);
        print_settings->put_PageHeight(height);
        print_settings->put_Orientation(orientation);

        std::error_code ec{};

        if (auto parent = settings.file.parent_path(); !fs::exists(parent))
        {
            fs::create_directories(parent, ec);
        }

        std::atomic_bool finished{false};

        auto complete_callback = [&](HRESULT, BOOL)
        {
            finished.store(true);
            return S_OK;
        };

        auto path     = fs::weakly_canonical(settings.file, ec);
        auto callback = Microsoft::WRL::Callback<ICoreWebView2PrintToPdfCompletedHandler>(complete_callback);

        webview->PrintToPdf(path.wstring().c_str(), print_settings.Get(), callback.Get());

        while (!finished)
        {
            m_parent->parent().run<false>();
        }
    }
} // namespace saucer::modules
