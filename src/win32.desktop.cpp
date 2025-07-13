#include "desktop.hpp"

#include "win32.utils.hpp"
#include "instantiate.hpp"

#include <ranges>

#include <windows.h>
#include <wrl.h>

#include <shobjidl_core.h>
#include <shldisp.h>

namespace saucer::modules
{
    using Microsoft::WRL::ComPtr;

    void desktop::open(const std::string &uri)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, uri] { return open(uri); });
        }

        ShellExecuteW(nullptr, L"open", utils::widen(uri).c_str(), nullptr, nullptr, SW_SHOWNORMAL);
    }

    template <picker::type Type>
    picker::result_t<Type> desktop::pick(const picker::options &opts)
    {
        if (!m_parent->thread_safe())
        {
            return m_parent->dispatch([this, opts] { return pick<Type>(opts); });
        }

        ComPtr<IFileOpenDialog> dialog;

        if (!SUCCEEDED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&dialog))))
        {
            return std::nullopt;
        }

        if (opts.initial)
        {
            ComPtr<IShellItem> item;
            SHCreateItemFromParsingName(opts.initial->wstring().c_str(), nullptr, IID_PPV_ARGS(&item));

            dialog->SetDefaultFolder(item.Get());
        }

        auto allowed = opts.filters                                                          //
                       | std::views::transform([](auto &&str) { return utils::widen(str); }) //
                       | std::views::join_with(L';')                                         //
                       | std::ranges::to<std::wstring>();

        COMDLG_FILTERSPEC filters[] = {{L"Allowed Files", allowed.c_str()}};
        dialog->SetFileTypes(1, filters);

        FILEOPENDIALOGOPTIONS options{};
        dialog->GetOptions(&options);

        if constexpr (Type == picker::type::files)
        {
            dialog->SetOptions(options | FOS_ALLOWMULTISELECT);
        }
        else if constexpr (Type == picker::type::folder)
        {
            dialog->SetOptions(options | FOS_PICKFOLDERS);
        }
        else if constexpr (Type == picker::type::save)
        {
            dialog->SetOptions(options & ~FOS_PATHMUSTEXIST & ~FOS_FILEMUSTEXIST);
        }

        dialog->Show(nullptr);

        ComPtr<IShellItemArray> results;

        if (!SUCCEEDED(dialog->GetResults(&results)))
        {
            return std::nullopt;
        }

        DWORD count{};

        if (!SUCCEEDED(results->GetCount(&count)))
        {
            return std::nullopt;
        }

        std::vector<fs::path> rtn;
        rtn.reserve(count);

        for (auto i = 0; count > i; ++i)
        {
            ComPtr<IShellItem> item;

            if (!SUCCEEDED(results->GetItemAt(i, &item)))
            {
                continue;
            }

            utils::string_handle path;

            if (!SUCCEEDED(item->GetDisplayName(SIGDN_FILESYSPATH, &path.reset())))
            {
                continue;
            }

            rtn.emplace_back(path.get());
        }

        if constexpr (Type == picker::type::files)
        {
            return rtn;
        }
        else
        {
            return rtn.front();
        }
    }

    INSTANTIATE_PICKER();
} // namespace saucer::modules
