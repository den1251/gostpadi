/* ОАиП ЛБ2 (вариант-образец): операторы цикла.
 * Требования: for/while/do-while, двойные циклы, break,
 * бесконечный цикл, #define вместо цифр, проверка ввода
 * с повторным запросом; math.h — только sqrt (корень). */
#include <stdio.h>
#include <math.h>

#define KOEF 3
#define LIMIT 5

int main(void) {
    int n;
    int i;
    double s;
    double r;
    double rost;
    double summa;
    int found;

    /* сумма 1+1/2+...+1/n превысила LIMIT: ищем первое такое n */
    do {
        printf("порог суммы (2..100): ");
        if (scanf("%lf", &rost) != 1 || rost < 2.0 || rost > 100.0) {
            printf("нужно число 2..100, повтори\n");
            rost = -1.0;
        }
        while (getchar() != '\n') {
        }
    } while (rost < 2.0 || rost > 100.0);

    s = 0.0;
    n = 0;
    while (s <= rost) {
        n = n + 1;
        s = s + 1.0 / n;
    }
    printf("сумма %f превысила порог на n = %d\n", s, n);

    /* бесконечный цикл: четырёхзначное число на единицу,
     * перестановка её в конец даёт число в KOEF раза больше */
    found = 0;
    i = 1000;
    while (1) {
        int rest;
        int shifted;
        rest = i % 1000;
        shifted = rest * 10 + 1;
        if (shifted == KOEF * i) {
            found = i;
            break;
        }
        i = i + 1;
        if (i > 9999) {
            break;
        }
    }
    if (found > 0) {
        printf("искомое число = %d\n", found);
    } else {
        printf("такого числа нет\n");
    }

    /* двойной цикл: суммы строк и кратные LIMIT */
    for (i = 1; i <= LIMIT; i = i + 1) {
        int j;
        int line_sum = 0;
        for (j = 1; j <= i; j = j + 1) {
            if (j % 2 == 0) {
                continue;
            }
            line_sum = line_sum + j * i;
        }
        printf("строка %d: сумма %d\n", i, line_sum);
    }

    /* do-while: гипотенуза после повторного ввода катетов */
    int kat1 = -1;
    int kat2 = -1;
    do {
        printf("катеты через пробел: ");
        if (scanf("%d%d", &kat1, &kat2) != 2) {
            printf("нужно два числа, повтори\n");
            kat1 = -1;
            kat2 = -1;
        }
        while (getchar() != '\n') {
        }
    } while (kat1 < 1 || kat2 < 1);
    summa = sqrt(kat1 * kat1 + kat2 * kat2);
    printf("гипотенуза = %f\n", summa);
    return 0;
}
