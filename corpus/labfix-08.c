/* ЛБ2, задача 5: цикл с накоплением.
 * Среднее арифметическое до стража 0. */
#include <stdio.h>

int main(void) {
    int x = 0;
    int count = 0;
    int sum = 0;

    printf("числа, 0 - конец ввода:\n");
    while (1) {
        printf("> ");
        if (scanf("%d", &x) != 1) {
            printf("нужно целое число\n");
            while (getchar() != '\n') {
            }
            continue;
        }
        if (x == 0) {
            break;
        }
        sum = sum + x;
        count = count + 1;
    }
    if (count == 0) {
        printf("чисел не было\n");
    } else {
        printf("сумма = %d, среднее = %.2lf\n", sum, (double)sum / count);
    }
    return 0;
}
