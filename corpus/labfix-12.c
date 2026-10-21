/* ЛБ4, задача 1: матрица 100x100 в функциях.
 * Меню: 1 - ввод, 2 - вывод, 3 - сумма всех элементов, 4 - выход. */
#include <stdio.h>

#define N_MAX 100

void fill_mas(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;

    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            printf("[%d][%d] = ", i, j);
            scanf("%d", &a[i][j]);
        }
    }
}

void show_mas(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;

    printf("матрица %dx%d:\n", n, m);
    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            printf("%4d", a[i][j]);
        }
        printf("\n");
    }
}

int sum_mas(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;
    int s = 0;

    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            s = s + a[i][j];
        }
    }
    return s;
}

int main(void) {
    int mas[N_MAX][N_MAX];
    int n = 0;
    int m = 0;
    int cmd = 0;

    while (1) {
        printf("1 - ввод, 2 - вывод, 3 - сумма, 0 - выход: ");
        if (scanf("%d", &cmd) != 1) {
            printf("нужна цифра\n");
            while (getchar() != '\n') {
            }
            continue;
        }
        if (cmd == 0) {
            break;
        }
        if (cmd == 1) {
            printf("размеры n m: ");
            scanf("%d %d", &n, &m);
            fill_mas(mas, n, m);
        } else if (cmd == 2) {
            show_mas(mas, n, m);
        } else if (cmd == 3) {
            printf("сумма = %d\n", sum_mas(mas, n, m));
        } else {
            printf("нет такой команды\n");
        }
    }
    return 0;
}
