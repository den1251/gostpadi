/* ОАиП ЛБ3 (вариант-образец): одномерные массивы.
 * Требования: статический массив 100, ручной ввод или rand,
 * одна функция минимум, почти полная проверка ввода,
 * вывод массива строкой после ввода, глобальные массивы запрещены. */
#include <stdio.h>

#define MAS_MAX 100

/* простая функция: приём по значению, возврат значения */
int sum_neg(int a[], int n) {
    int i;
    int s = 0;
    for (i = 0; i < n; i = i + 1) {
        if (a[i] < 0) {
            s = s + a[i];
        }
    }
    return s;
}

/* передача массива в функцию: void, оба значения-параметра */
void print_mas(const int a[], int n) {
    int i;
    printf("mas[^\n");
    for (i = 0; i < n; i = i + 1) {
        printf("%d ", a[i]);
    }
    printf("]\nmas[%d]\n", n);
}

/* вывод после ввода, либо меню (выбор пользователя) */
int main(void) {
    int mas[MAS_MAX];
    int n = 0;
    int c;
    int rnd = 0;

    do {
        printf("число элементов (1..%d): ", MAS_MAX);
        if (scanf("%d", &n) != 1 || n < 1 || n > MAS_MAX) {
            printf("нужно число 1..%d, повтори\n", MAS_MAX);
            n = 0;
        }
        while (getchar() != '\n') {
        }
    } while (n < 1);

    do {
        printf("1 - ввести вручную, 2 - случайные: ");
        if (scanf("%d", &c) != 1) {
            printf("нужно число, повтори\n");
            c = -1;
        }
        while (getchar() != '\n') {
        }
    } while (c < 1 || c > 2);

    if (c == 2) {
        rnd = 1;
    }

    if (rnd) {
        int i;
        int seed;
        do {
            printf("сдвиг (1..1000): ");
            if (scanf("%d", &seed) != 1 || seed < 1) {
                printf("нужно число, повтори\n");
                seed = -1;
            }
            while (getchar() != '\n') {
            }
        } while (seed < 1);
        for (i = 0; i < n; i = i + 1) {
            mas[i] = (seed * (i + 7)) % 100 - 50;
        }
    } else {
        int i;
        for (i = 0; i < n; i = i + 1) {
            do {
                printf("mas[%d]: ", i);
                if (scanf("%d", &mas[i]) != 1) {
                    printf("нужно число, повтори\n");
                    mas[i] = -9999;
                }
                while (getchar() != '\n') {
                }
            } while (mas[i] == -9999);
        }
    }

    print_mas(mas, n);
    printf("сумма отрицательных = %d\n", sum_neg(mas, n));
    return 0;
}
