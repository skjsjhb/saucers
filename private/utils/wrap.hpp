#pragma once

#include "icon.hpp"
#include "webview.hpp"
#include "navigation.hpp"

namespace bindings
{
    template <typename T>
    struct wrap
    {
        using type = T;

        template <typename U>
        static auto convert(U &data)
        {
            return data;
        };
    };

    template <>
    struct wrap<const std::string &>
    {
        using type = const char *;

        template <typename U>
        static auto convert(U &data)
        {
            return data.c_str();
        };
    };

    template <>
    struct wrap<const saucer::icon &>
    {
        using type = saucer_icon *;

        template <typename U>
        static auto convert(U &data)
        {
            // ! User is responsible for freeing this!
            return saucer_icon::make(std::move(data));
        };
    };

    template <>
    struct wrap<const saucer::navigation &>
    {
        using type = saucer_navigation *;

        template <typename U>
        static auto convert(U &data)
        {
            // ! User is responsible for freeing this!
            return saucer_navigation::make(std::move(data));
        };
    };

    template <typename R, typename... Ts>
    struct wrap<std::function<R(Ts...)>>
    {
        using type = wrap<R>::type (*)(saucer_handle *, typename wrap<Ts>::type...);

        static auto convert(void *callback)
        {
            return reinterpret_cast<type>(callback);
        }
    };

    template <typename T>
    T callback(saucer_handle *handle, void *callback)
    {
        return [handle, callback]<typename... Ts>(Ts &&...args)
        {
            auto *converted = wrap<T>::convert(callback);
            return std::invoke(converted, handle, wrap<Ts>::convert(args)...);
        };
    };
} // namespace bindings
