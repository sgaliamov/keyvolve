# Blog Post Title

i want to write a blog post about my findings about designing my own keyboard layout.

The structure will be:
1. Introduction
1. ToC and description
1. Step by step guide how to use modes
1. QMK tricks
1. Conclusion

## Introduction

I made my first DIY keyboard.

[Image of DIY keyboard](path/to/your/image.jpg)

And I decided that a perfect keyboard need a perfect layout.

I made an attempt already a couple of years ago. But I realized that there is no perfect layout. Comfortable keyboard is the most important factor.

One of the fastest typists use qwerty after all.

Одна из главных причин почему я начал этот проект - уменьшить нагрузку на мизинцы.
Мизинец самый слабый палец. Неразумно нагружать его так же как и указательный или средний.

## Main Content

I will write it as a guide. How to start from scratch and design your own keyboard layout.

## Метрики

Что определяет хорошую раскладку клавиатуры?

Скорость печати, конечно же. Но не менее важна эргономика и комфорт при длительном наборе текста.
Мы же не хотим быть крабами.
По-этому важен баланс и распределение нагрузки между руками и пальцами.

По-этому первой метрикой будет **баланс нагрузки между руками** `effortsImbalance` (название колонки в цсв и в конфиге).

На практике оказалось, что введения просто баланса недостаточно. Нужно явно указать, что нужен еще физический баланс. Для этого была введена метрика `handsImbalance`

Еще мы не хотим, чтобы пальцы ходили верх вниз слишком часто. Когда пальцы находятся на домашнем ряду, печатать гораздо удобнее и быстрее.
А когда рука смещается вверх или вниз, удобно, если следующая буква будет находиться в том же ряду.
Попробуйте сами - вытяните какой-нибудь палец на верхний или нижний ряд и увидьте как остальные пальцы смещаются вместе с ним.
По-этому следующей метрикой будет **частота переключения между рядами**.

Изначально для этого у меня был всего один параметр задавать это `rowSwitchRatio`. Но, например, для такой раскладки оказалось, что какой-то палец получает большую нагрузку по сравнению с другими.
Иногда даже мизинец оказывается перегружен, чего я хотел избежать. (добавить пример такой раскладки)
По-этому пришлось отказаться от одного параметра и ввести несколько метрик для оценки нагрузки на каждый палец (`pinkyRowSwitchRatio`, `ringRowSwitchRatio`, `middleRowSwitchRatio`, `indexRowSwitchRatio`).

Я оставил `rowSwitchRatio` на тот случай если кому-то покажется что задавать это для каждого пальца слишком сложно, и если я захочу явно контролировать общую частоту переключения между рядами.

Очень важным решением был выбор последовательного непрерывного нажатия клавиш одной рукой.
Есть даже исследования на эту тему:
- [On the One Hand or on the Other: Trade-Off in Timing Precision in Bimanual Musical Scale Playing](https://pmc.ncbi.nlm.nih.gov/articles/PMC6737297)
- [Speed invariance of independent control of finger movements in pianists](https://pmc.ncbi.nlm.nih.gov/articles/PMC3545004)

Мы конечно же не пианисты, но мы также нажимаем на клавиши.
Согласитесь же что очень удобно что на кверти Е и Р оказались рядом.

Если это действительно правильная догадка, то раскладки тип дворак просто неэффективны, и даже хуже чем qwerty.

Для этого я ввел метрику `handSwitchRatio`.
При желании можно сделать дворак-стиль раскладки, учитывая метрику `handSwitchRatio`.


### Methodology

Why bigrams.

намного быстрее напечатать пару букв, если они расположены на одной руке и удобно расположены для пальцев.

Why lower hand switch.

Why balance.

Stress on pinky.


### Modes

First you need to get statistics about english letter frequency and bigram frequency.

I used [TBD]

### Configuration options

See placement rules: [placement-rules.md](C:/Users/Admin/projects/keyvolve/docs/placement-rules.md).

## Conclusion

Why create one perfect keyboard layout when everyone can design their own?

I have own biases, for example, put ER and TH on one hand, and I committed to the idea to maximizing bigram efficiency.
But someone may find it ridiculous.

But perfection is not about the performance. It's the journey itself.

### Set by step guide how to use modes

### Other languages and QMK

1. the problem with other languages
1. fast switch
1. the problem with ctrl
1. vim from ru (TEMP_EN)
   некоторые символы отсутствуют на русской раскладке и приходится переключаться на английскую. Например, #. Тут-то приходит на помощь эта магическая кнопка.
   Еще иногда нужно возвращаться в режим редактирования в Vim и нужно нажать `i` непереключаясь на английскую раскладку.

## Acknowledgements

list used websites and resources here.

## TODO

clean not used modes
