/* ОАиП ЛБ1 (вариант-образец): линейные + разветвляющиеся алгоритмы.
 * Требования: полная проверка ввода с повторным запросом; вывод
 * с точностью 2-3 знака; без string.h/структур; один return 0. */
#include <stdio.h>

int main(void) {
    const double PI = 3.141593;
    int h;
    int r;
    double s_side;
    double s_full;
    double v;
    int met;
    int month;

    do {
        printf("высота цилиндра (1..1000): ");
        if (scanf("%d", &h) != 1) {
            printf("нужно число, повтори ввод\n");
            h = -1;
        }
        while (getchar() != '\n') {
        }
    } while (h < 1 || h > 1000);

    do {
        printf("радиус цилиндра (1..1000): ");
        if (scanf("%d", &r) != 1) {
            printf("нужно число, повтори ввод\n");
            r = -1;
        }
        while (getchar() != '\n') {
        }
    } while (r < 1 || r > 1000);

    s_side = 2.0 * PI * r * h;
    s_full = s_side + 2.0 * PI * r * r;
    v = s_side * r / 2.0;
    printf("боковая %.2f, полная %.2f, объем %.2f\n", s_side, s_full, v);

    do {
        printf("1 - сравнить два числа, 2 - месяц, 3 - конец: ");
        if (scanf("%d", &met) != 1) {
            printf("нужно число\n");
            met = -1;
        }
        while (getchar() != '\n') {
        }
    } while (met < 1 || met > 3);

    if (met == 1) {
        int a;
        int b;
        printf("два числа: ");
        scanf("%d%d", &a, &b);
        if (a == b) {
            printf("равны\n");
        } else {
            printf("не равны\n");
        }
    }

    if (met == 2) {
        do {
            printf("номер месяца (1..12): ");
            if (scanf("%d", &month) != 1) {
                printf("нужно число\n");
                month = -1;
            }
            while (getchar() != '\n') {
            }
        } while (month < 1 || month > 12);
        switch (month) {
            case 1: printf("январь, 31 день\n"); break;
            case 2: printf("февраль, 28 дней\n"); break;
            case 3: printf("март, 31 день\n"); break;
            case 4: printf("апрель, 30 дней\n"); break;
            default: printf("остальные месяцы\n"); break;
        }
    }

    return 0;
}
