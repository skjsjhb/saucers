#pragma once

#include "stash.hpp"
#include "../utils/overload.hpp"

#include <functional>

namespace saucer
{
    template <typename T>
    stash<T>::stash(variant_t data) : m_data(std::move(data))
    {
    }

    template <typename T>
    const T *stash<T>::data() const
    {
        overload visitor = {
            [](const lazy_t &data) { return data.get()->data(); },
            [](const auto &data) { return data.data(); },
        };

        return std::visit(visitor, m_data);
    }

    template <typename T>
    std::size_t stash<T>::size() const
    {
        overload visitor = {
            [](const lazy_t &data) { return data.get()->size(); },
            [](const auto &data) { return data.size(); },
        };

        return std::visit(visitor, m_data);
    }

    template <typename T>
    stash<T> stash<T>::from(owning_t data)
    {
        return {std::move(data)};
    }

    template <typename T>
    stash<T> stash<T>::view(viewing_t data)
    {
        return {std::move(data)};
    }

    template <typename T>
    stash<T> stash<T>::lazy(lazy_t data)
    {
        return {std::move(data)};
    }

    template <typename T>
    template <typename Callback>
    stash<T> stash<T>::lazy(Callback callback)
    {
        auto fn = [callback = std::move(callback)]
        {
            return std::make_shared<stash>(std::invoke(callback));
        };

        return {std::async(std::launch::deferred, std::move(fn)).share()};
    }

    template <typename T>
    stash<T> stash<T>::empty()
    {
        return {{}};
    }

    template <typename T, typename V>
        requires std::ranges::range<V>
    auto make_stash(const V &data)
    {
        if constexpr (std::ranges::view<V>)
        {
            return stash<T>::view({std::begin(data), std::end(data)});
        }
        else
        {
            return stash<T>::from({std::begin(data), std::end(data)});
        }
    }
} // namespace saucer
