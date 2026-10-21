/* ЛБ4, задача 3: меню и обработка строк матрицы.
 * Ручной ввод, суммы по строкам, поиск столбца с максимумом. */
#include <stdio.h>

#define N_MAX 100

void read_mas(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;

    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            printf("a[%d][%d] = ", i, j);
            scanf("%d", &a[i][j]);
        }
    }
}

void show_mas(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;

    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            printf("%5d", a[i][j]);
        }
        printf("\n");
    }
}

void row_sums(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;
    int s = 0;

    for (i = 0; i < n; i++) {
        s = 0;
        for (j = 0; j < m; j++) {
            s = s + a[i][j];
        }
        printf("строка %d: сумма %d\n", i, s);
    }
}

int max_col(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;
    int best = 0;
    int best_sum = 0;
    int s = 0;

    for (j = 0; j < m; j++) {
        s = 0;
        for (i = 0; i < n; i++) {
            s = s + a[i][j];
        }
        if (j == 0 || s > best_sum) {
            best_sum = s;
            best = j;
        }
    }
    printf("столбец %d, сумма %d\n", best, best_sum);
    return best;
}

int main(void) {
    int mas[N_MAX][N_MAX];
    int n = 0;
    int m = 0;
    int cmd = 0;
    int filled = 0;

    while (1) {
        printf("1 - ввод, 2 - вывод, 3 - суммы строк, 4 - столбец max, 0 - выход: ");
        if (scanf("%d", &cmd) != 1) {
            printf("нужна цифра\n");
            while (getchar() != '\n') {
            }
            continue;
        }
        if (cmd == 0) {
            break;
        }
        if (cmd < 1 || cmd > 4) {
            printf("нет такой команды\n");
            continue;
        }
        if (!filled && cmd != 1) {
            printf("сначала введите матрицу\n");
            continue;
        }
        if (cmd == 1) {
            printf("n m: ");
            scanf("%d %d", &n, &m);
            read_mas(mas, n, m);
            filled = 1;
        } else if (cmd == 2) {
            show_mas(mas, n, m);
        } else if (cmd == 3) {
            row_sums(mas, n, m);
        } else {
            max_col(mas, n, m);
        }
    }
    return 0;
}
