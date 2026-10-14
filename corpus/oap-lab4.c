/* ОАиП ЛБ4 (вариант-образец): многомерные массивы.
 * Требования: полная проверка ввода с повторным запросом,
 * большинство кода в функциях, второй параметр — константа,
 * меню программы (1 ввод, 2 вывод, 3 вычисление), корректный вывод
 * матрицы (mas[2][3], строки по одной статической плашке),
 * статический массив 100x100, глобальные массивы запрещены. */
#include <stdio.h>

#define N_MAX 100

/* вывод матрицы: второй параметр — константа размера */
void show_mas(const int a[][N_MAX], const int n, const int m) {
    int i;
    int j;
    printf("mas[%d][%d]:\n", n, m);
    for (i = 0; i < n; i = i + 1) {
        for (j = 0; j < m; j = j + 1) {
            printf("%d ", a[i][j]);
        }
        printf("\n");
    }
}

/* ввод матрицы с полной проверкой ввода */
void fill_mas(int a[][N_MAX], int n, int m) {
    int i;
    int j;
    for (i = 0; i < n; i = i + 1) {
        for (j = 0; j < m; j = j + 1) {
            do {
                printf("[%d][%d]: ", i, j);
                if (scanf("%d", &a[i][j]) != 1) {
                    printf("нужно число, повтори\n");
                    a[i][j] = -9999;
                }
                while (getchar() != '\n') {
                }
            } while (a[i][j] == -9999);
        }
    }
}

/* вычисление: максимум главной диагонали */
int max_diag(const int a[][N_MAX], const int n) {
    int i;
    int best = a[0][0];
    for (i = 1; i < n; i = i + 1) {
        if (a[i][i] > best) {
            best = a[i][i];
        }
    }
    return best;
}

int main(void) {
    int mas[N_MAX][N_MAX];
    int n = 0;
    int m = 0;
    int r;
    int done = 0;
    int filled = 0;

    do {
        printf("размеры n m (1..%d): ", N_MAX);
        if (scanf("%d%d", &n, &m) != 2 || n < 1 || n > N_MAX || m < 1 || m > N_MAX) {
            printf("нужно два числа 1..%d, повтори\n", N_MAX);
            n = 0;
            m = 0;
        }
        while (getchar() != '\n') {
        }
    } while (n < 1 || m < 1);

    /* меню: 1 ввод, 2 вывод, 3 вычисление, 4 конец */
    while (!done) {
        do {
            printf("1 - ввод, 2 - вывод, 3 - вычислить max диагонали, 4 - конец: ");
            if (scanf("%d", &r) != 1 || r < 1 || r > 4) {
                printf("нужно число 1..4, повтори\n");
                r = 0;
            }
            while (getchar() != '\n') {
            }
        } while (r < 1);

        if (r == 1) {
            fill_mas(mas, n, m);
            filled = 1;
        }
        if (r == 2) {
            if (!filled) {
                printf("массив не заполнен\n");
            } else {
                show_mas(mas, n, m);
            }
        }
        if (r == 3) {
            if (!filled) {
                printf("массив не заполнен\n");
            } else {
                printf("max диагонали = %d\n", max_diag(mas, n));
            }
        }
        if (r == 4) {
            done = 1;
        }
    }
    return 0;
}
